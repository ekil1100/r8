/*---
description: Nested var declarations are instantiated before Script execution.
esid: sec-variable-statement
flags: [raw]
---*/
x; { var x = 3; } x;
