'use client';

import { useState, useEffect } from 'react';
import { ethers } from 'ethers';

interface TransactionStatus {
  id: string;
  txHash?: string;
  status: 'pending' | 'confirmed' | 'failed';
  error?: string;
  blockNumber?: number;
  gasUsed?: string;
}

interface QueueStatus {
  queueLength: number;
  currentProcessing: number;
  maxConcurrent: number;
  processingTransactions: string[];
}

export default function Home() {
  const [wallet, setWallet] = useState<any>(null);
  const [transactionId, setTransactionId] = useState<string>('');
  const [transactionStatus, setTransactionStatus] = useState<TransactionStatus | null>(null);
  const [queueStatus, setQueueStatus] = useState<QueueStatus | null>(null);
  const [ws, setWs] = useState<WebSocket | null>(null);

  // 连接钱包
  const connectWallet = async () => {
    if (typeof window.ethereum !== 'undefined') {
      try {
        const provider = new ethers.BrowserProvider(window.ethereum);
        const signer = await provider.getSigner();
        const address = await signer.getAddress();
        setWallet({ provider, signer, address });
      } catch (error) {
        console.error('连接钱包失败:', error);
      }
    } else {
      alert('请安装 MetaMask 钱包');
    }
  };

  // 提交交易
  const submitTransaction = async () => {
    if (!wallet) {
      alert('请先连接钱包');
      return;
    }

    try {
      const to = '0x742d35Cc6634C0532925a3b8D0C0C2C4C4C4C4C4'; // 示例地址
      const amount = '0.001';
      const timestamp = Date.now();
      const message = `${wallet.address}:${to}:${amount}:${timestamp}`;
      
      // 签名消息
      const signature = await wallet.signer.signMessage(message);
      
      // 提交到中继器
      const response = await fetch('http://localhost:3001/api/transaction/submit', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          from: wallet.address,
          to,
          amount,
          signature,
          message,
          nonce: Date.now().toString(),
          timestamp,
          apiKey: 'your-api-key',
          clientId: wallet.address,
        }),
      });

      const result = await response.json();
      if (result.success) {
        setTransactionId(result.transactionId);
        // 订阅交易状态更新
        subscribeToTransaction(result.transactionId);
      } else {
        alert('交易提交失败: ' + result.error);
      }
    } catch (error) {
      console.error('交易提交错误:', error);
      alert('交易提交失败');
    }
  };

  // 订阅交易状态
  const subscribeToTransaction = (txId: string) => {
    if (ws) {
      ws.send(JSON.stringify({
        type: 'subscribe_transaction',
        transactionId: txId,
      }));
    }
  };

  // 查询交易状态
  const checkTransactionStatus = async () => {
    if (!transactionId) return;
    
    try {
      const response = await fetch(`http://localhost:3001/api/transaction/${transactionId}/status`);
      const status = await response.json();
      setTransactionStatus(status);
    } catch (error) {
      console.error('查询交易状态失败:', error);
    }
  };

  // 获取队列状态
  const fetchQueueStatus = async () => {
    try {
      const response = await fetch('http://localhost:3001/api/queue/status');
      const status = await response.json();
      setQueueStatus(status);
    } catch (error) {
      console.error('获取队列状态失败:', error);
    }
  };

  // WebSocket 连接
  useEffect(() => {
    const websocket = new WebSocket('ws://localhost:3001');
    
    websocket.onopen = () => {
      console.log('WebSocket 连接已建立');
      setWs(websocket);
    };
    
    websocket.onmessage = (event) => {
      const data = JSON.parse(event.data);
      if (data.type === 'transaction_update') {
        setTransactionStatus(data.status);
      }
    };
    
    websocket.onclose = () => {
      console.log('WebSocket 连接已关闭');
      setWs(null);
    };

    return () => {
      websocket.close();
    };
  }, []);

  // 定期更新队列状态
  useEffect(() => {
    const interval = setInterval(fetchQueueStatus, 5000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="min-h-screen bg-gray-50 py-8">
      <div className="max-w-4xl mx-auto px-4">
        <div className="bg-white rounded-lg shadow-lg p-6 mb-6">
          <h1 className="text-3xl font-bold text-gray-900 mb-6 text-center">
            x402 中继器服务
          </h1>
          
          {/* 钱包连接 */}
          <div className="mb-6">
            <h2 className="text-xl font-semibold mb-4">钱包连接</h2>
            {wallet ? (
              <div className="bg-green-50 border border-green-200 rounded-lg p-4">
                <p className="text-green-800">
                  已连接钱包: <span className="font-mono">{wallet.address}</span>
                </p>
              </div>
            ) : (
              <button
                onClick={connectWallet}
                className="bg-blue-600 text-white px-6 py-2 rounded-lg hover:bg-blue-700 transition-colors"
              >
                连接 MetaMask 钱包
              </button>
            )}
          </div>

          {/* 交易提交 */}
          <div className="mb-6">
            <h2 className="text-xl font-semibold mb-4">提交交易</h2>
            <button
              onClick={submitTransaction}
              disabled={!wallet}
              className="bg-green-600 text-white px-6 py-2 rounded-lg hover:bg-green-700 transition-colors disabled:bg-gray-400 disabled:cursor-not-allowed"
            >
              提交测试交易
            </button>
          </div>

          {/* 交易状态 */}
          {transactionStatus && (
            <div className="mb-6">
              <h2 className="text-xl font-semibold mb-4">交易状态</h2>
              <div className="bg-gray-50 border border-gray-200 rounded-lg p-4">
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <span className="font-semibold">交易ID:</span>
                    <p className="font-mono text-sm">{transactionStatus.id}</p>
                  </div>
                  <div>
                    <span className="font-semibold">状态:</span>
                    <span className={`ml-2 px-2 py-1 rounded text-sm ${
                      transactionStatus.status === 'confirmed' ? 'bg-green-100 text-green-800' :
                      transactionStatus.status === 'failed' ? 'bg-red-100 text-red-800' :
                      'bg-yellow-100 text-yellow-800'
                    }`}>
                      {transactionStatus.status}
                    </span>
                  </div>
                  {transactionStatus.txHash && (
                    <div className="col-span-2">
                      <span className="font-semibold">交易哈希:</span>
                      <p className="font-mono text-sm">{transactionStatus.txHash}</p>
                    </div>
                  )}
                  {transactionStatus.blockNumber && (
                    <div>
                      <span className="font-semibold">区块号:</span>
                      <p>{transactionStatus.blockNumber}</p>
                    </div>
                  )}
                  {transactionStatus.gasUsed && (
                    <div>
                      <span className="font-semibold">Gas 使用量:</span>
                      <p>{transactionStatus.gasUsed}</p>
                    </div>
                  )}
                  {transactionStatus.error && (
                    <div className="col-span-2">
                      <span className="font-semibold text-red-600">错误信息:</span>
                      <p className="text-red-600">{transactionStatus.error}</p>
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* 队列状态 */}
          {queueStatus && (
            <div className="mb-6">
              <h2 className="text-xl font-semibold mb-4">队列状态</h2>
              <div className="bg-gray-50 border border-gray-200 rounded-lg p-4">
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <span className="font-semibold">队列长度:</span>
                    <p className="text-2xl font-bold text-blue-600">{queueStatus.queueLength}</p>
                  </div>
                  <div>
                    <span className="font-semibold">正在处理:</span>
                    <p className="text-2xl font-bold text-green-600">
                      {queueStatus.currentProcessing}/{queueStatus.maxConcurrent}
                    </p>
                  </div>
                  {queueStatus.processingTransactions.length > 0 && (
                    <div className="col-span-2">
                      <span className="font-semibold">处理中的交易:</span>
                      <div className="mt-2 space-y-1">
                        {queueStatus.processingTransactions.map((txId) => (
                          <p key={txId} className="font-mono text-sm bg-yellow-100 p-2 rounded">
                            {txId}
                          </p>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* 手动查询按钮 */}
          <div className="text-center">
            <button
              onClick={checkTransactionStatus}
              disabled={!transactionId}
              className="bg-gray-600 text-white px-6 py-2 rounded-lg hover:bg-gray-700 transition-colors disabled:bg-gray-400 disabled:cursor-not-allowed"
            >
              手动查询交易状态
            </button>
          </div>
        </div>

        {/* 系统信息 */}
        <div className="bg-white rounded-lg shadow-lg p-6">
          <h2 className="text-xl font-semibold mb-4">系统信息</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <h3 className="font-semibold mb-2">WebSocket 连接状态</h3>
              <span className={`px-2 py-1 rounded text-sm ${
                ws ? 'bg-green-100 text-green-800' : 'bg-red-100 text-red-800'
              }`}>
                {ws ? '已连接' : '未连接'}
              </span>
            </div>
            <div>
              <h3 className="font-semibold mb-2">服务状态</h3>
              <span className="px-2 py-1 rounded text-sm bg-green-100 text-green-800">
                运行中
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
