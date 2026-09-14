use crate::Value;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Instruction {
    Number(f64),
    Positive,
    Negative,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

pub(crate) fn run(code: &[Instruction]) -> Value {
    let mut stack: Vec<f64> = Vec::with_capacity(code.len());
    for instruction in code {
        match *instruction {
            Instruction::Number(number) => stack.push(number),
            Instruction::Positive => {}
            Instruction::Negative => {
                let value = stack
                    .last_mut()
                    .expect("Compiler must provide a unary operand");
                *value = -*value;
            }
            operation => {
                let right = stack.pop().expect("Compiler must provide a right operand");
                let left = stack
                    .last_mut()
                    .expect("Compiler must provide a left operand");
                *left = match operation {
                    Instruction::Add => *left + right,
                    Instruction::Subtract => *left - right,
                    Instruction::Multiply => *left * right,
                    Instruction::Divide => *left / right,
                    Instruction::Remainder => *left % right,
                    _ => unreachable!("Non-binary instructions were handled above"),
                };
            }
        }
    }
    assert!(
        stack.len() <= 1,
        "Compiler must leave at most one completion value"
    );
    stack.pop().map_or(Value::Undefined, Value::Number)
}
