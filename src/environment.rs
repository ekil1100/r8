use std::collections::BTreeMap;

use crate::{Error, ErrorKind, Script, Value, vm};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeclarationKind {
    Var,
    Let,
    Const,
}

#[derive(Clone, Debug)]
pub(crate) struct Declaration {
    pub name: String,
    pub kind: DeclarationKind,
    pub offset: usize,
}

#[derive(Debug)]
pub(crate) struct Binding {
    pub kind: DeclarationKind,
    // None denotes the temporal dead zone, not the JS value undefined.
    pub value: Option<Value>,
}

pub(crate) type Environment = BTreeMap<String, Binding>;

/// Global bindings shared by Scripts executed in this context.
#[derive(Debug, Default)]
pub struct Context {
    pub(crate) globals: Environment,
}

impl Context {
    pub fn eval(&mut self, source: &str) -> Result<Value, Error> {
        self.run(&Script::parse(source)?)
    }

    pub fn run(&mut self, script: &Script) -> Result<Value, Error> {
        self.instantiate(script)?;
        vm::run(script, self)
    }

    fn instantiate(&mut self, script: &Script) -> Result<(), Error> {
        // Validate all declarations before changing the global environment.
        for declaration in &script.lexical {
            if self.globals.contains_key(&declaration.name)
                || global_constant(&declaration.name).is_some()
            {
                return Err(redeclaration(declaration));
            }
        }
        for declaration in &script.variables {
            if let Some(binding) = self.globals.get(&declaration.name) {
                if binding.kind != DeclarationKind::Var {
                    return Err(redeclaration(declaration));
                }
            } else if unsupported_global(&declaration.name) {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    declaration.offset,
                    "Declarations involving this global builtin are not implemented.",
                ));
            }
        }
        for declaration in &script.lexical {
            self.globals.insert(
                declaration.name.clone(),
                Binding {
                    kind: declaration.kind,
                    value: None,
                },
            );
        }
        for declaration in &script.variables {
            self.globals
                .entry(declaration.name.clone())
                .or_insert(Binding {
                    kind: DeclarationKind::Var,
                    value: Some(global_constant(&declaration.name).unwrap_or(Value::Undefined)),
                });
        }
        Ok(())
    }

    pub(crate) fn read(&self, name: &str, offset: usize) -> Result<Value, Error> {
        if let Some(binding) = self.globals.get(name) {
            return read_binding(binding, name, offset);
        }
        if let Some(value) = global_constant(name) {
            return Ok(value);
        }
        let (kind, message) = if unsupported_global(name) {
            (
                ErrorKind::Unsupported,
                format!("Global builtin '{name}' is not implemented."),
            )
        } else {
            (ErrorKind::Reference, format!("'{name}' is not defined."))
        };
        Err(Error::new(kind, offset, message))
    }

    pub(crate) fn set_var(&mut self, name: &str, value: Value) {
        // The three global value properties are non-writable in sloppy Scripts.
        if global_constant(name).is_none() {
            self.globals
                .get_mut(name)
                .expect("Variable must be instantiated")
                .value = Some(value);
        }
    }
}

pub(crate) fn read_binding(binding: &Binding, name: &str, offset: usize) -> Result<Value, Error> {
    binding.value.ok_or_else(|| {
        Error::new(
            ErrorKind::Reference,
            offset,
            format!("Cannot access '{name}' before initialization."),
        )
    })
}

fn redeclaration(declaration: &Declaration) -> Error {
    Error::new(
        ErrorKind::Syntax,
        declaration.offset,
        format!(
            "Global binding '{}' has already been declared or is restricted.",
            declaration.name
        ),
    )
}

fn global_constant(name: &str) -> Option<Value> {
    match name {
        "undefined" => Some(Value::Undefined),
        "NaN" => Some(Value::Number(f64::NAN)),
        "Infinity" => Some(Value::Number(f64::INFINITY)),
        _ => None,
    }
}

fn unsupported_global(name: &str) -> bool {
    matches!(
        name,
        "globalThis"
            | "eval"
            | "isFinite"
            | "isNaN"
            | "parseFloat"
            | "parseInt"
            | "decodeURI"
            | "decodeURIComponent"
            | "encodeURI"
            | "encodeURIComponent"
            | "Object"
            | "Function"
            | "Boolean"
            | "Symbol"
            | "Number"
            | "BigInt"
            | "Math"
            | "Date"
            | "String"
            | "RegExp"
            | "Array"
            | "Map"
            | "Set"
            | "WeakMap"
            | "WeakSet"
            | "ArrayBuffer"
            | "SharedArrayBuffer"
            | "DataView"
            | "Atomics"
            | "JSON"
            | "Promise"
            | "Reflect"
            | "Proxy"
            | "WeakRef"
            | "FinalizationRegistry"
            | "Iterator"
            | "Error"
            | "AggregateError"
            | "EvalError"
            | "RangeError"
            | "ReferenceError"
            | "SyntaxError"
            | "TypeError"
            | "URIError"
            | "Int8Array"
            | "Uint8Array"
            | "Uint8ClampedArray"
            | "Int16Array"
            | "Uint16Array"
            | "Int32Array"
            | "Uint32Array"
            | "Float16Array"
            | "Float32Array"
            | "Float64Array"
            | "BigInt64Array"
            | "BigUint64Array"
            | "DisposableStack"
            | "AsyncDisposableStack"
            | "SuppressedError"
    )
}
