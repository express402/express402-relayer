import { ethers } from 'ethers';
import { redisClient } from './redis';
import * as jwt from 'jsonwebtoken';
import * as bcrypt from 'bcryptjs';

export interface SecurityConfig {
  jwtSecret: string;
  apiKey: string;
  maxRequestsPerMinute: number;
  minBalanceThreshold: string;
  maxTransactionAmount: string;
}

export class SecurityManager {
  private config: SecurityConfig;

  constructor() {
    this.config = {
      jwtSecret: process.env.JWT_SECRET || 'default-secret',
      apiKey: process.env.API_KEY || 'default-api-key',
      maxRequestsPerMinute: 100,
      minBalanceThreshold: process.env.MIN_BALANCE_THRESHOLD || '0.01',
      maxTransactionAmount: process.env.MAX_TRANSACTION_AMOUNT || '1.0',
    };
  }

  // JWT Token 验证
  async verifyToken(token: string): Promise<any> {
    try {
      return jwt.verify(token, this.config.jwtSecret);
    } catch (error) {
      throw new Error('Invalid token');
    }
  }

  async generateToken(payload: any): Promise<string> {
    return jwt.sign(payload, this.config.jwtSecret, { expiresIn: '1h' });
  }

  // API Key 验证
  async verifyApiKey(apiKey: string): Promise<boolean> {
    return apiKey === this.config.apiKey;
  }

  // 请求频率限制
  async checkRateLimit(clientId: string): Promise<boolean> {
    const key = `rate_limit:${clientId}`;
    const current = await redisClient.get(key);
    
    if (!current) {
      await redisClient.set(key, 1, 60); // 1分钟过期
      return true;
    }
    
    if (current >= this.config.maxRequestsPerMinute) {
      return false;
    }
    
    await redisClient.set(key, current + 1, 60);
    return true;
  }

  // 签名验证
  async validateSignature(message: string, signature: string, expectedAddress: string): Promise<boolean> {
    try {
      const recoveredAddress = ethers.verifyMessage(message, signature);
      return recoveredAddress.toLowerCase() === expectedAddress.toLowerCase();
    } catch (error) {
      return false;
    }
  }

  // 防重放攻击
  async checkReplayAttack(nonce: string, timestamp: number): Promise<boolean> {
    const key = `nonce:${nonce}`;
    const exists = await redisClient.exists(key);
    
    if (exists) {
      return true; // 重放攻击
    }
    
    // 检查时间戳是否在合理范围内（5分钟内）
    const now = Date.now();
    if (Math.abs(now - timestamp) > 300000) {
      return true; // 时间戳过期
    }
    
    await redisClient.set(key, true, 300); // 5分钟过期
    return false;
  }

  // 余额检查
  async checkBalance(address: string, amount: string): Promise<boolean> {
    const balance = await redisClient.get(`balance:${address}`);
    if (!balance) {
      return false;
    }
    
    const balanceEth = parseFloat(balance);
    const amountEth = parseFloat(amount);
    const threshold = parseFloat(this.config.minBalanceThreshold);
    
    return balanceEth >= amountEth + threshold;
  }

  // 交易金额限制
  async checkTransactionAmount(amount: string): Promise<boolean> {
    const amountEth = parseFloat(amount);
    const maxAmount = parseFloat(this.config.maxTransactionAmount);
    
    return amountEth <= maxAmount;
  }

  // 预充值机制
  async checkPrepaidBalance(clientId: string, amount: string): Promise<boolean> {
    const key = `prepaid:${clientId}`;
    const balance = await redisClient.get(key);
    
    if (!balance) {
      return false;
    }
    
    const balanceEth = parseFloat(balance);
    const amountEth = parseFloat(amount);
    
    return balanceEth >= amountEth;
  }

  async deductPrepaidBalance(clientId: string, amount: string): Promise<void> {
    const key = `prepaid:${clientId}`;
    const balance = await redisClient.get(key);
    
    if (balance) {
      const balanceEth = parseFloat(balance);
      const amountEth = parseFloat(amount);
      const newBalance = balanceEth - amountEth;
      
      await redisClient.set(key, newBalance.toString(), 86400); // 24小时过期
    }
  }

  async addPrepaidBalance(clientId: string, amount: string): Promise<void> {
    const key = `prepaid:${clientId}`;
    const balance = await redisClient.get(key);
    
    const balanceEth = balance ? parseFloat(balance) : 0;
    const amountEth = parseFloat(amount);
    const newBalance = balanceEth + amountEth;
    
    await redisClient.set(key, newBalance.toString(), 86400); // 24小时过期
  }

  // 回滚策略
  async createRollbackPoint(transactionId: string, state: any): Promise<void> {
    const key = `rollback:${transactionId}`;
    await redisClient.set(key, state, 3600); // 1小时过期
  }

  async executeRollback(transactionId: string): Promise<any> {
    const key = `rollback:${transactionId}`;
    const state = await redisClient.get(key);
    
    if (state) {
      // 执行回滚逻辑
      await this.performRollback(state);
      await redisClient.del(key);
    }
    
    return state;
  }

  private async performRollback(state: any): Promise<void> {
    // 实现具体的回滚逻辑
    console.log('Executing rollback for state:', state);
  }

  // 密码哈希
  async hashPassword(password: string): Promise<string> {
    return bcrypt.hash(password, 10);
  }

  async comparePassword(password: string, hash: string): Promise<boolean> {
    return bcrypt.compare(password, hash);
  }

  // 安全检查综合验证
  async performSecurityCheck(request: any): Promise<{ valid: boolean; error?: string }> {
    try {
      // 1. API Key 验证
      if (!await this.verifyApiKey(request.apiKey)) {
        return { valid: false, error: 'Invalid API key' };
      }

      // 2. 频率限制检查
      if (!await this.checkRateLimit(request.clientId)) {
        return { valid: false, error: 'Rate limit exceeded' };
      }

      // 3. 重放攻击检查
      if (await this.checkReplayAttack(request.nonce, request.timestamp)) {
        return { valid: false, error: 'Replay attack detected' };
      }

      // 4. 签名验证
      if (!await this.validateSignature(request.message, request.signature, request.from)) {
        return { valid: false, error: 'Invalid signature' };
      }

      // 5. 交易金额检查
      if (!await this.checkTransactionAmount(request.amount)) {
        return { valid: false, error: 'Transaction amount exceeds limit' };
      }

      // 6. 预充值余额检查
      if (!await this.checkPrepaidBalance(request.clientId, request.amount)) {
        return { valid: false, error: 'Insufficient prepaid balance' };
      }

      return { valid: true };
    } catch (error) {
      return { valid: false, error: 'Security check failed' };
    }
  }
}
