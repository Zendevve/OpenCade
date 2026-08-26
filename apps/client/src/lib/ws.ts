import { createEnvelope, parseEnvelope, type Envelope } from "@opencade/protocol";

export type ConnectionState = "idle" | "connecting" | "open" | "reconnecting" | "closed";
export type MessageListener = (message: Envelope<unknown>) => void;

export function reconnectDelay(attempt: number): number {
  return Math.min(30_000, 500 * 2 ** Math.max(0, attempt));
}

export class OpenCadeSocket {
  private socket: WebSocket | null = null;
  private attempt = 0;
  private stopped = false;
  private reconnectTimer: number | null = null;
  private listeners = new Set<MessageListener>();
  private pending = new Map<
    string,
    { resolve: (message: Envelope<unknown>) => void; reject: (error: Error) => void; timer: number }
  >();

  constructor(
    private readonly baseUrl: string,
    private readonly token: string,
    private readonly onState: (state: ConnectionState) => void = () => undefined
  ) {}

  connect(): void {
    if (
      this.socket?.readyState === WebSocket.OPEN ||
      this.socket?.readyState === WebSocket.CONNECTING
    ) {
      return;
    }
    this.stopped = false;
    if (this.reconnectTimer !== null) window.clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
    this.open(this.attempt === 0 ? "connecting" : "reconnecting");
  }

  close(): void {
    this.stopped = true;
    if (this.reconnectTimer !== null) window.clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
    this.socket?.close(1000, "client closed");
    this.socket = null;
    this.rejectPending(new Error("WebSocket closed"));
    this.onState("closed");
  }

  subscribe(listener: MessageListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  send(type: string, payload: unknown, timeoutMs = 8_000): Promise<Envelope<unknown>> {
    if (this.socket?.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("WebSocket is not connected"));
    }
    const envelope = createEnvelope(type, payload);
    this.socket.send(JSON.stringify(envelope));
    return new Promise((resolve, reject) => {
      const timer = window.setTimeout(() => {
        this.pending.delete(envelope.request_id);
        reject(new Error("WebSocket request timed out"));
      }, timeoutMs);
      this.pending.set(envelope.request_id, { resolve, reject, timer });
    });
  }

  private open(state: ConnectionState): void {
    if (this.stopped) return;
    this.onState(state);
    const url = new URL("/ws", this.baseUrl.replace(/^http/, "ws"));
    const socket = new WebSocket(url, ["opencade.v1", `opencade.auth.${this.token}`]);
    this.socket = socket;
    socket.onopen = () => {
      if (this.socket !== socket) return;
      this.attempt = 0;
      this.onState("open");
    };
    socket.onmessage = (event) => {
      if (this.socket !== socket) return;
      if (typeof event.data !== "string") return;
      try {
        const envelope = parseEnvelope(event.data);
        const pending = this.pending.get(envelope.request_id);
        if (pending) {
          window.clearTimeout(pending.timer);
          this.pending.delete(envelope.request_id);
          pending.resolve(envelope);
        }
        this.listeners.forEach((listener) => listener(envelope));
      } catch {
        // The server owns protocol validation; malformed frames are ignored client-side.
      }
    };
    socket.onerror = () => socket.close();
    socket.onclose = () => {
      if (this.socket !== socket) return;
      this.socket = null;
      this.rejectPending(new Error("WebSocket connection was lost"));
      if (this.stopped) return;
      const delay = reconnectDelay(this.attempt++);
      this.onState("reconnecting");
      this.reconnectTimer = window.setTimeout(() => {
        this.reconnectTimer = null;
        if (!this.stopped) this.open("reconnecting");
      }, delay);
    };
  }

  private rejectPending(error: Error): void {
    this.pending.forEach(({ reject, timer }) => {
      window.clearTimeout(timer);
      reject(error);
    });
    this.pending.clear();
  }
}
