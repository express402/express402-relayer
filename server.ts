import express from 'express';
import cors from 'cors';
import helmet from 'helmet';
import { v4 as uuidv4 } from 'uuid';
import { WalletPool } from './lib/wallet-pool';
import { TransactionQueue } from './lib/transaction-queue';
import { SecurityManager } from './lib/security';
import { redisClient } from './lib/redis';

const app = express();
const port = process.env.PORT || 3001;

// 中间件
app.use(helmet());
app.use(cors());
app.use(express.json());

// 初始化服务
const walletPool = new WalletPool(process.env.RPC_URL || '');
const transactionQueue = new TransactionQueue(walletPool);
const securityManager = new SecurityManager();

// 启动队列处理
setInterval(async () => {
  await transactionQueue.processQueue();
}, 1000); // 每秒处理一次

// API 路由

// 健康检查
app.get('/health', async (req, res) => {
  try {
    const queueStatus = await transactionQueue.getQueueStatus();
    const wallets = walletPool.getAllWallets();
    
    res.json({
      status: 'healthy',
      timestamp: new Date().toISOString(),
      queue: queueStatus,
      wallets: wallets.length,
      activeWallets: wallets.filter(w => w.isActive).length
    });
  } catch (error) {
    res.status(500).json({ error: 'Health check failed' });
  }
});

// 提交交易请求
app.post('/api/transaction/submit', async (req, res) => {
  try {
    const {
      from,
      to,
      amount,
      signature,
      message,
      nonce,
      timestamp,
      apiKey,
      clientId
    } = req.body;

    // 安全检查
    const securityCheck = await securityManager.performSecurityCheck({
      apiKey,
      clientId,
      nonce,
      timestamp,
      message,
      signature,
      from,
      amount
    });

    if (!securityCheck.valid) {
      return res.status(400).json({ error: securityCheck.error });
    }

    // 创建交易请求
    const transactionRequest = {
      id: uuidv4(),
      from,
      to,
      amount,
      signature,
      timestamp: Date.now(),
      priority: 1,
      retryCount: 0
    };

    // 添加到队列
    await transactionQueue.addTransaction(transactionRequest);

    // 扣除预充值余额
    await securityManager.deductPrepaidBalance(clientId, amount);

    res.json({
      success: true,
      transactionId: transactionRequest.id,
      message: 'Transaction submitted successfully'
    });

  } catch (error) {
    console.error('Transaction submission error:', error);
    res.status(500).json({ error: 'Transaction submission failed' });
  }
});

// 查询交易状态
app.get('/api/transaction/:id/status', async (req, res) => {
  try {
    const { id } = req.params;
    const status = await transactionQueue.getTransactionStatus(id);
    
    if (!status) {
      return res.status(404).json({ error: 'Transaction not found' });
    }
    
    res.json(status);
  } catch (error) {
    res.status(500).json({ error: 'Failed to get transaction status' });
  }
});

// 获取队列状态
app.get('/api/queue/status', async (req, res) => {
  try {
    const status = await transactionQueue.getQueueStatus();
    res.json(status);
  } catch (error) {
    res.status(500).json({ error: 'Failed to get queue status' });
  }
});

// 钱包管理
app.get('/api/wallets', async (req, res) => {
  try {
    const wallets = walletPool.getAllWallets();
    res.json(wallets);
  } catch (error) {
    res.status(500).json({ error: 'Failed to get wallets' });
  }
});

// 预充值
app.post('/api/prepaid/add', async (req, res) => {
  try {
    const { clientId, amount, apiKey } = req.body;
    
    if (!await securityManager.verifyApiKey(apiKey)) {
      return res.status(401).json({ error: 'Invalid API key' });
    }
    
    await securityManager.addPrepaidBalance(clientId, amount);
    
    res.json({
      success: true,
      message: 'Prepaid balance added successfully'
    });
  } catch (error) {
    res.status(500).json({ error: 'Failed to add prepaid balance' });
  }
});

// 查询预充值余额
app.get('/api/prepaid/:clientId/balance', async (req, res) => {
  try {
    const { clientId } = req.params;
    const balance = await redisClient.get(`prepaid:${clientId}`);
    
    res.json({
      clientId,
      balance: balance || '0'
    });
  } catch (error) {
    res.status(500).json({ error: 'Failed to get prepaid balance' });
  }
});

// 回滚交易
app.post('/api/transaction/:id/rollback', async (req, res) => {
  try {
    const { id } = req.params;
    const { apiKey } = req.body;
    
    if (!await securityManager.verifyApiKey(apiKey)) {
      return res.status(401).json({ error: 'Invalid API key' });
    }
    
    const rollbackState = await securityManager.executeRollback(id);
    
    res.json({
      success: true,
      rollbackState,
      message: 'Rollback executed successfully'
    });
  } catch (error) {
    res.status(500).json({ error: 'Failed to execute rollback' });
  }
});

// WebSocket 支持
import { WebSocketServer } from 'ws';
import { createServer } from 'http';

const server = createServer(app);
const wss = new WebSocketServer({ server });

wss.on('connection', (ws) => {
  console.log('WebSocket client connected');
  
  ws.on('message', async (message) => {
    try {
      const data = JSON.parse(message.toString());
      
      if (data.type === 'subscribe_transaction') {
        // 订阅交易状态更新
        const interval = setInterval(async () => {
          const status = await transactionQueue.getTransactionStatus(data.transactionId);
          if (status) {
            ws.send(JSON.stringify({
              type: 'transaction_update',
              transactionId: data.transactionId,
              status
            }));
            
            if (status.status === 'confirmed' || status.status === 'failed') {
              clearInterval(interval);
            }
          }
        }, 2000); // 每2秒检查一次
        
        ws.on('close', () => {
          clearInterval(interval);
        });
      }
    } catch (error) {
      ws.send(JSON.stringify({ error: 'Invalid message format' }));
    }
  });
  
  ws.on('close', () => {
    console.log('WebSocket client disconnected');
  });
});

// 启动服务器
server.listen(port, () => {
  console.log(`x402 Relayer server running on port ${port}`);
});

export default app;
