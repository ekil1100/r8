/*---
description: Block lexical bindings do not escape their block.
esid: sec-block
flags: [raw]
negative:
  phase: runtime
  type: ReferenceError
---*/
{ let local = 1; } local;
