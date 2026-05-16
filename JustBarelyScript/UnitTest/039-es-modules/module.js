export const GREETING = "Hello from module";

export function add(a, b) {
  return a + b;
}

export class Point {
  constructor(x, y) {
    this.x = x;
    this.y = y;
  }
  toString() {
    return "(" + this.x + "," + this.y + ")";
  }
}

export default function defaultFn() {
  return "default export";
}
