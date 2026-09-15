/*---
description: Declaration lists initialize bindings in source order.
esid: sec-let-and-const-declarations
flags: [raw]
---*/
let left = 3; const right = 2; var result = left + right; result;
