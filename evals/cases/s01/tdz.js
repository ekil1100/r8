/*---
description: A block binding shadows the outer binding throughout its temporal dead zone.
esid: sec-let-and-const-declarations
flags: [raw]
negative:
  phase: runtime
  type: ReferenceError
---*/
let x = 1; { x; let x = 2; }
