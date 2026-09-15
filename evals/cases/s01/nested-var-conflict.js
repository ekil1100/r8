/*---
description: A nested var declaration conflicts with an enclosing lexical declaration.
esid: sec-block-static-semantics-early-errors
flags: [raw]
negative:
  phase: parse
  type: SyntaxError
---*/
{ let x; { var x; } }
