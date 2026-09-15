/*---
description: Duplicate lexical bindings are early errors.
esid: sec-let-and-const-declarations
flags: [raw]
negative:
  phase: parse
  type: SyntaxError
---*/
missing; let x; let x;
