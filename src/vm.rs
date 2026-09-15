use crate::{
    Context, Error, Script, Value,
    environment::{Binding, Declaration, Environment, read_binding},
};

#[derive(Debug)]
pub(crate) enum Instruction {
    Number(f64),
    Undefined,
    Load(String, usize),
    Initialize(String),
    SetVar(String),
    Complete,
    EnterScope(Vec<Declaration>),
    LeaveScope,
    Positive,
    Negative,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

pub(crate) fn run(script: &Script, context: &mut Context) -> Result<Value, Error> {
    let mut stack: Vec<Value> = Vec::new();
    let mut completion = Value::Undefined;
    let mut scopes: Vec<Environment> = Vec::new();
    for instruction in &script.code {
        match instruction {
            Instruction::Number(number) => stack.push(Value::Number(*number)),
            Instruction::Undefined => stack.push(Value::Undefined),
            Instruction::EnterScope(declarations) => {
                scopes.push(
                    declarations
                        .iter()
                        .map(|declaration| {
                            (
                                declaration.name.clone(),
                                Binding {
                                    kind: declaration.kind,
                                    value: None,
                                },
                            )
                        })
                        .collect(),
                );
            }
            Instruction::LeaveScope => {
                scopes.pop().expect("Block scopes must be balanced");
            }
            Instruction::Load(name, offset) => {
                let value = match scopes.iter().rev().find_map(|scope| scope.get(name)) {
                    Some(binding) => read_binding(binding, name, *offset)?,
                    None => context.read(name, *offset)?,
                };
                stack.push(value);
            }
            Instruction::Initialize(name) => {
                let value = stack.pop().expect("Compiler must provide an initializer");
                let scope = scopes.last_mut().unwrap_or(&mut context.globals);
                let binding = scope.get_mut(name).expect("Binding must be instantiated");
                assert!(binding.value.is_none(), "Lexical bindings initialize once");
                binding.value = Some(value);
            }
            Instruction::SetVar(name) => {
                let value = stack.pop().expect("Compiler must provide an initializer");
                context.set_var(name, value);
            }
            Instruction::Complete => {
                completion = stack
                    .pop()
                    .expect("Expression must leave a completion value");
                assert!(stack.is_empty(), "Statement must balance the operand stack");
            }
            Instruction::Positive | Instruction::Negative => {
                let value = stack
                    .last_mut()
                    .expect("Compiler must provide a unary operand");
                let number = to_number(*value);
                *value = Value::Number(if matches!(instruction, Instruction::Negative) {
                    -number
                } else {
                    number
                });
            }
            operation => {
                let right = to_number(stack.pop().expect("Compiler must provide a right operand"));
                let left = stack
                    .last_mut()
                    .expect("Compiler must provide a left operand");
                let number = to_number(*left);
                *left = Value::Number(match operation {
                    Instruction::Add => number + right,
                    Instruction::Subtract => number - right,
                    Instruction::Multiply => number * right,
                    Instruction::Divide => number / right,
                    Instruction::Remainder => number % right,
                    _ => unreachable!("Non-binary instructions were handled above"),
                });
            }
        }
    }
    assert!(scopes.is_empty(), "Script must balance the scope stack");
    assert!(stack.is_empty(), "Script must balance the operand stack");
    Ok(completion)
}

fn to_number(value: Value) -> f64 {
    match value {
        Value::Number(number) => number,
        Value::Undefined => f64::NAN,
    }
}
