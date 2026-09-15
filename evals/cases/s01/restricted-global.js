/*---
description: Global lexical declarations cannot shadow restricted global value properties.
esid: sec-globaldeclarationinstantiation
flags: [raw]
negative:
  phase: runtime
  type: SyntaxError
---*/
let undefined = 1;
