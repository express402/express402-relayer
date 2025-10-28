// x402 Relayer SDK
export interface RelayerConfig {
  apiUrl: string;
  apiKey: string;
  wsUrl?: string;
}

export interface TransactionRequest {
  from: string;
  to: string;
  amount: string;
  signature: string;
  message: string;
  nonce: string;
  timestamp: number;
  clientId: string;
}

export interface TransactionResponse {
  success: boolean;
  transactionId?: string;
  error?: string;
  message?: string;
}

export interface TransactionStatus {
  id: string;
  txHash?: string;
  status: 'pending' | 'confirmed' | 'failed';
  error?: string;
  blockNumber?: number;
  gasUsed?: string;
}

export interface QueueStatus {
  queueLength: number;
  currentProcessing: number;
  maxConcurrent: number;
  processingTransactions: string[];
}

export interface WalletInfo {
  address: string;
  balance: string;
  nonce: number;
  isActive: boolean;
  lastUsed: number;
}

export class X402RelayerSDK {
  private config: RelayerConfig;
  private ws: WebSocket | null = null;
  private eventListeners: Map<string, Function[]> = new Map();

  constructor(config: RelayerConfig) {
    this.config = config;
  }

  // 提交交易
  async submitTransaction(request: TransactionRequest): Promise<TransactionResponse> {
    const response = await fetch(`${this.config.apiUrl}/api/transaction/submit`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        ...request,
        apiKey: this.config.apiKey,
      }),
    });

    return await response.json();
  }

  // 查询交易状态
  async getTransactionStatus(transactionId: string): Promise<TransactionStatus> {
    const response = await fetch(`${this.config.apiUrl}/api/transaction/${transactionId}/status`);
    return await response.json();
  }

  // 获取队列状态
  async getQueueStatus(): Promise<QueueStatus> {
    const response = await fetch(`${this.config.apiUrl}/api/queue/status`);
    return await response.json();
  }

  // 获取钱包信息
  async getWallets(): Promise<WalletInfo[]> {
    const response = await fetch(`${this.config.apiUrl}/api/wallets`);
    return await response.json();
  }

  // 添加预充值
  async addPrepaidBalance(clientId: string, amount: string): Promise<any> {
    const response = await fetch(`${this.config.apiUrl}/api/prepaid/add`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        clientId,
        amount,
        apiKey: this.config.apiKey,
      }),
    });

    return await response.json();
  }

  // 查询预充值余额
  async getPrepaidBalance(clientId: string): Promise<any> {
    const response = await fetch(`${this.config.apiUrl}/api/prepaid/${clientId}/balance`);
    return await response.json();
  }

  // 回滚交易
  async rollbackTransaction(transactionId: string): Promise<any> {
    const response = await fetch(`${this.config.apiUrl}/api/transaction/${transactionId}/rollback`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        apiKey: this.config.apiKey,
      }),
    });

    return await response.json();
  }

  // WebSocket 连接
  connectWebSocket(): void {
    if (this.config.wsUrl) {
      this.ws = new WebSocket(this.config.wsUrl);
      
      this.ws.onopen = () => {
        this.emit('connected');
      };
      
      this.ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        this.emit('message', data);
      };
      
      this.ws.onclose = () => {
        this.emit('disconnected');
      };
      
      this.ws.onerror = (error) => {
        this.emit('error', error);
      };
    }
  }

  // 订阅交易状态
  subscribeToTransaction(transactionId: string): void {
    if (this.ws) {
      this.ws.send(JSON.stringify({
        type: 'subscribe_transaction',
        transactionId,
      }));
    }
  }

  // 事件监听
  on(event: string, callback: Function): void {
    if (!this.eventListeners.has(event)) {
      this.eventListeners.set(event, []);
    }
    this.eventListeners.get(event)!.push(callback);
  }

  // 移除事件监听
  off(event: string, callback: Function): void {
    const listeners = this.eventListeners.get(event);
    if (listeners) {
      const index = listeners.indexOf(callback);
      if (index > -1) {
        listeners.splice(index, 1);
      }
    }
  }

  // 触发事件
  private emit(event: string, ...args: any[]): void {
    const listeners = this.eventListeners.get(event);
    if (listeners) {
      listeners.forEach(callback => callback(...args));
    }
  }

  // 断开连接
  disconnect(): void {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }
}

// 工具函数
export class RelayerUtils {
  // 生成消息
  static generateMessage(from: string, to: string, amount: string, timestamp: number): string {
    return `${from}:${to}:${amount}:${timestamp}`;
  }

  // 生成 nonce
  static generateNonce(): string {
    return Date.now().toString() + Math.random().toString(36).substr(2, 9);
  }

  // 验证地址
  static isValidAddress(address: string): boolean {
    return /^0x[a-fA-F0-9]{40}$/.test(address);
  }

  // 格式化金额
  static formatAmount(amount: string, decimals: number = 18): string {
    return (parseFloat(amount) * Math.pow(10, decimals)).toString();
  }

  // 解析金额
  static parseAmount(amount: string, decimals: number = 18): string {
    return (parseFloat(amount) / Math.pow(10, decimals)).toString();
  }
}

// 默认配置
export const defaultConfig: RelayerConfig = {
  apiUrl: 'http://localhost:3001',
  apiKey: 'your-api-key',
  wsUrl: 'ws://localhost:3001',
};

export default X402RelayerSDK;
