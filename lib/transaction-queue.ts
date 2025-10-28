import { ethers } from 'ethers';
import { redisClient } from './redis';
import { WalletPool } from './wallet-pool';

export interface TransactionRequest {
  id: string;
  from: string;
  to: string;
  amount: string;
  signature: string;
  timestamp: number;
  priority: number;
  retryCount: number;
}

export interface TransactionResult {
  id: string;
  txHash?: string;
  status: 'pending' | 'confirmed' | 'failed';
  error?: string;
  blockNumber?: number;
  gasUsed?: string;
}

export class TransactionQueue {
  private walletPool: WalletPool;
  private processingQueue: Map<string, TransactionRequest> = new Map();
  private maxConcurrent: number;
  private currentProcessing: number = 0;

  constructor(walletPool: WalletPool) {
    this.walletPool = walletPool;
    this.maxConcurrent = parseInt(process.env.MAX_CONCURRENT_TRANSACTIONS || '5');
  }

  async addTransaction(request: TransactionRequest): Promise<void> {
    // 验证签名
    if (!await this.validateSignature(request)) {
      throw new Error('Invalid signature');
    }

    // 检查重放攻击
    if (await this.isReplayAttack(request)) {
      throw new Error('Replay attack detected');
    }

    // 存储到 Redis
    await redisClient.set(`tx:${request.id}`, request, 3600); // 1小时过期
    await redisClient.lpush('transaction_queue', request);

    console.log(`Transaction ${request.id} added to queue`);
  }

  async processQueue(): Promise<void> {
    if (this.currentProcessing >= this.maxConcurrent) {
      return;
    }

    const request = await redisClient.rpop('transaction_queue');
    if (!request) {
      return;
    }

    this.currentProcessing++;
    this.processingQueue.set(request.id, request);

    try {
      await this.executeTransaction(request);
    } catch (error) {
      console.error(`Transaction ${request.id} failed:`, error);
      await this.handleFailedTransaction(request, error as Error);
    } finally {
      this.currentProcessing--;
      this.processingQueue.delete(request.id);
    }
  }

  private async executeTransaction(request: TransactionRequest): Promise<void> {
    const wallet = await this.walletPool.getAvailableWallet();
    if (!wallet) {
      throw new Error('No available wallet');
    }

    try {
      // 锁定钱包
      await this.walletPool.lockWallet(wallet.address);

      // 构建交易
      const tx = await this.buildTransaction(request, wallet);
      
      // 发送交易
      const walletInstance = new ethers.Wallet(wallet.privateKey, this.walletPool['provider']);
      const txResponse = await walletInstance.sendTransaction(tx);
      
      // 更新状态
      await this.updateTransactionStatus(request.id, {
        id: request.id,
        txHash: txResponse.hash,
        status: 'pending'
      });

      // 等待确认
      const receipt = await txResponse.wait();
      
      await this.updateTransactionStatus(request.id, {
        id: request.id,
        txHash: txResponse.hash,
        status: 'confirmed',
        blockNumber: receipt?.blockNumber,
        gasUsed: receipt?.gasUsed.toString()
      });

      console.log(`Transaction ${request.id} confirmed in block ${receipt?.blockNumber}`);

    } finally {
      // 释放钱包
      await this.walletPool.releaseWallet(wallet.address);
    }
  }

  private async buildTransaction(request: TransactionRequest, wallet: any): Promise<any> {
    const gasPrice = await this.walletPool.getGasPrice();
    const gasLimit = parseInt(process.env.GAS_LIMIT || '21000');

    return {
      to: request.to,
      value: ethers.parseEther(request.amount),
      gasLimit: gasLimit,
      gasPrice: gasPrice,
      nonce: wallet.nonce,
    };
  }

  private async validateSignature(request: TransactionRequest): Promise<boolean> {
    try {
      const message = `${request.from}:${request.to}:${request.amount}:${request.timestamp}`;
      const recoveredAddress = ethers.verifyMessage(message, request.signature);
      return recoveredAddress.toLowerCase() === request.from.toLowerCase();
    } catch (error) {
      return false;
    }
  }

  private async isReplayAttack(request: TransactionRequest): Promise<boolean> {
    const key = `replay:${request.from}:${request.timestamp}`;
    const exists = await redisClient.exists(key);
    
    if (!exists) {
      await redisClient.set(key, true, 300); // 5分钟过期
      return false;
    }
    
    return true;
  }

  private async handleFailedTransaction(request: TransactionRequest, error: Error): Promise<void> {
    request.retryCount++;
    
    if (request.retryCount < 3) {
      // 重新加入队列
      await redisClient.lpush('transaction_queue', request);
      console.log(`Transaction ${request.id} retry ${request.retryCount}`);
    } else {
      // 标记为失败
      await this.updateTransactionStatus(request.id, {
        id: request.id,
        status: 'failed',
        error: error.message
      });
    }
  }

  private async updateTransactionStatus(id: string, result: TransactionResult): Promise<void> {
    await redisClient.set(`tx_result:${id}`, result, 3600);
  }

  async getTransactionStatus(id: string): Promise<TransactionResult | null> {
    return await redisClient.get(`tx_result:${id}`);
  }

  async getQueueStatus(): Promise<any> {
    const queueLength = await redisClient.llen('transaction_queue');
    return {
      queueLength,
      currentProcessing: this.currentProcessing,
      maxConcurrent: this.maxConcurrent,
      processingTransactions: Array.from(this.processingQueue.keys())
    };
  }
}
