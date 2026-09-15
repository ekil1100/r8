/*---
description: Escaped reserved words cannot be binding identifiers.
esid: sec-identifiers-static-semantics-early-errors
flags: [raw]
negative:
  phase: parse
  type: SyntaxError
---*/
let \u0069f = 1;
