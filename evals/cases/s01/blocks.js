/*---
description: Block lexical bindings do not replace outer bindings.
esid: sec-block
flags: [raw]
---*/
let x = 1; { const x = 2; x; } x;
