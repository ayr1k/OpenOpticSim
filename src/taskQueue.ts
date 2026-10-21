export interface JobView {
  id: number;
  label: string;
  state: "queued" | "running" | "done" | "cancelled" | "failed";
  started: number;
  detail: string;
}
export class TaskQueue {
  private serial = 0;
  private active = 0;
  private limit = 1;
  private paused = false;
  private listeners = new Set<() => void>();
  private waiting: (() => void)[] = [];
  private controllers = new Map<number, AbortController>();
  private rows: JobView[] = [];
  subscribe = (f: () => void) => {
    this.listeners.add(f);
    return () => {
      this.listeners.delete(f);
    };
  };
  snapshot = () => this.rows;
  private emit() {
    this.rows = [...this.rows];
    this.listeners.forEach((f) => f());
  }
  setLimit(n: number) {
    this.limit = Math.max(1, Math.min(2, n));
    this.drain();
    this.emit();
  }
  setPaused(value: boolean) {
    this.paused = value;
    this.drain();
    this.emit();
  }
  cancel(id: number) {
    this.controllers.get(id)?.abort();
  }
  private drain() {
    while (!this.paused && this.active < this.limit && this.waiting.length)
      this.waiting.shift()!();
  }
  async run<T>(
    label: string,
    signal: AbortSignal,
    work: (signal: AbortSignal, detail: (text: string) => void) => Promise<T>,
  ): Promise<T> {
    if (signal.aborted) throw new DOMException("Cancelled", "AbortError");
    const id = ++this.serial,
      ctrl = new AbortController();
    const row: JobView = {
      id,
      label,
      state: "queued",
      started: Date.now(),
      detail: "等待计算资源",
    };
    this.rows = [
      ...this.rows.filter(
        (r) => r.state === "queued" || r.state === "running" || r.id > id - 20,
      ),
      row,
    ];
    this.controllers.set(id, ctrl);
    this.emit();
    const abort = () => ctrl.abort();
    signal.addEventListener("abort", abort, { once: true });
    return new Promise<T>((resolve, reject) => {
      let begun = false,
        finished = false;
      const finish = (state: JobView["state"], error?: unknown) => {
        if (finished) return;
        finished = true;
        row.state = state;
        row.detail = error ? String(error) : "";
        this.controllers.delete(id);
        signal.removeEventListener("abort", abort);
        if (begun) this.active--;
        this.emit();
        this.drain();
      };
      ctrl.signal.addEventListener(
        "abort",
        () => {
          if (!begun) {
            finish("cancelled");
            reject(new DOMException("Cancelled", "AbortError"));
          }
        },
        { once: true },
      );
      this.waiting.push(() => {
        if (finished) return;
        begun = true;
        this.active++;
        row.state = "running";
        row.detail = "计算中";
        this.emit();
        void Promise.resolve()
          .then(() =>
            work(ctrl.signal, (text) => {
              row.detail = text;
              this.emit();
            }),
          )
          .then(
            (value) => {
              if (ctrl.signal.aborted) {
                finish("cancelled");
                reject(new DOMException("Cancelled", "AbortError"));
              } else {
                finish("done");
                resolve(value);
              }
            },
            (error) => {
              finish(ctrl.signal.aborted ? "cancelled" : "failed", error);
              reject(error);
            },
          );
      });
      if (signal.aborted) ctrl.abort();
      this.drain();
    });
  }
}
export const tasks = new TaskQueue();
