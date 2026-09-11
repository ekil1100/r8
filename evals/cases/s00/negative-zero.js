/*---
description: Negative zero.
esid: sec-unary-minus-operator
---*/
assert.sameValue(1 / -0, -Infinity);
assert.notSameValue(-0, 0);
