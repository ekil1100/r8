/*---
description: A const declaration requires an initializer.
esid: sec-let-and-const-declarations
flags: [raw]
negative:
  phase: parse
  type: SyntaxError
---*/
const x;
