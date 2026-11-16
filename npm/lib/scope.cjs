'use strict';
// Caller-owned synchronous scope-exit reporting, the native EventOnDrop. Rust
// delivers on drop; JavaScript's deterministic scope exit is `using` (or an
// explicit `dispose()` in `finally`). Garbage collection never delivers.
const { DisposedError } = require('./errors.cjs');
class EventOnDrop {
  #event; #handler; #armed = true;
  constructor(event, handler) {
    if (typeof handler !== 'function') throw new TypeError('handler must be a function');
    if (handler.constructor.name === 'AsyncFunction') throw new TypeError('EventOnDrop handlers must be synchronous');
    this.#event = event; this.#handler = handler;
    Object.freeze(this);
  }
  get armed() { return this.#armed; }
  /** The armed event, or undefined after disarming. */
  get event() { return this.#armed ? this.#event : undefined; }
  /** Update the armed event's correlation after accepted progress. */
  set event(value) {
    if (!this.#armed) throw new DisposedError('A disarmed EventOnDrop cannot be rearmed');
    this.#event = value;
  }
  /** Release the event and handler without delivering; repeated calls do nothing. */
  disarm() { this.#armed = false; this.#event = undefined; this.#handler = undefined; }
  /** Deliver the armed event once; a disarmed or delivered guard does nothing. */
  dispose() {
    if (!this.#armed) return;
    const event = this.#event; const handler = this.#handler;
    this.disarm();
    const result = handler(event);
    if (result !== null && result !== undefined && typeof result.then === 'function') throw new TypeError('EventOnDrop handlers must return synchronously, not a Promise');
    if (result !== undefined) throw new TypeError('EventOnDrop handlers must return undefined; handle every outcome explicitly');
  }
  [Symbol.dispose]() { this.dispose(); }
}
module.exports = { EventOnDrop };
