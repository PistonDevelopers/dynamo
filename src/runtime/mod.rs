//! Dyon runtime.

use std::sync::Arc;
use std::collections::HashMap;
use rand;
use range::Range;

use ast;
use intrinsics;
use embed;

use FnIndex;
use Module;
use Variable;
use UnsafeRef;
use TINVOTS;

mod for_n;
mod for_in;

/// Which side an expression is evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Whether to insert key in object when missing.
    LeftInsert(bool),
    /// Evaluating right side of assignment.
    Right
}

/// Stores return flow, used to continue executing, return, break out of loop or continue loop.
#[derive(Debug)]
pub enum Flow {
    /// Continues execution.
    Continue,
    /// Return from function.
    Return,
    /// Break loop, with optional label.
    Break(Option<Arc<String>>),
    /// Continue loop, with optional label.
    ContinueLoop(Option<Arc<String>>),
}

/// Stores function calls.
#[derive(Debug)]
pub struct Call {
    // was .0
    fn_name: Arc<String>,
    /// The index of the relative function in module.
    pub(crate) index: usize,
    file: Option<Arc<String>>,
    // was .1
    stack_len: usize,
    // was .2
    local_len: usize,
    current_len: usize,
}

lazy_static! {
    pub(crate) static ref text_type: Arc<String> = Arc::new("string".into());
    pub(crate) static ref f64_type: Arc<String> = Arc::new("number".into());
    pub(crate) static ref vec4_type: Arc<String> = Arc::new("vec4".into());
    pub(crate) static ref mat4_type: Arc<String> = Arc::new("mat4".into());
    pub(crate) static ref return_type: Arc<String> = Arc::new("return".into());
    pub(crate) static ref bool_type: Arc<String> = Arc::new("boolean".into());
    pub(crate) static ref object_type: Arc<String> = Arc::new("object".into());
    pub(crate) static ref link_type: Arc<String> = Arc::new("link".into());
    pub(crate) static ref array_type: Arc<String> = Arc::new("array".into());
    pub(crate) static ref unsafe_ref_type: Arc<String> = Arc::new("unsafe_ref".into());
    pub(crate) static ref ref_type: Arc<String> = Arc::new("ref".into());
    pub(crate) static ref rust_object_type: Arc<String> = Arc::new("rust_object".into());
    pub(crate) static ref option_type: Arc<String> = Arc::new("option".into());
    pub(crate) static ref result_type: Arc<String> = Arc::new("result".into());
    pub(crate) static ref thread_type: Arc<String> = Arc::new("thread".into());
    pub(crate) static ref closure_type: Arc<String> = Arc::new("closure".into());
    pub(crate) static ref in_type: Arc<String> = Arc::new("in".into());
}

/// Stores data needed for running a Dyon program.
pub struct Runtime {
    /// Stores variables on the stack.
    pub stack: Vec<Variable>,
    /// name, file, stack_len, local_len.
    pub call_stack: Vec<Call>,
    /// Stores stack of locals.
    pub local_stack: Vec<(Arc<String>, usize)>,
    /// Stores stack of current objects.
    ///
    /// When a current object is used, the runtime searches backwards
    /// until it finds the last current variable with the name.
    pub current_stack: Vec<(Arc<String>, usize)>,
    ret: Arc<String>,
    pub(crate) rng: rand::rngs::StdRng,
}

#[inline(always)]
fn resolve<'a>(stack: &'a Vec<Variable>, var: &'a Variable) -> &'a Variable {
    match *var {
        Variable::Ref(ind) => &stack[ind],
        _ => var
    }
}

// Looks up an item from a variable property.
fn item_lookup(
    module: &Module,
    var: *mut Variable,
    stack: &mut [Variable],
    call_stack: &[Call],
    prop: &ast::Id,
    start_stack_len: usize,
    expr_j: &mut usize,
    insert: bool, // Whether to insert key in object.
    last: bool,   // Whether it is the last property.
) -> Result<*mut Variable, String> {
    use ast::Id;
    use std::collections::hash_map::Entry;

    unsafe {
        match *var {
            Variable::Object(ref mut obj) => {
                let id = match prop {
                    &Id::String(_, ref id) => id.clone(),
                    &Id::Expression(_) => {
                        let id = start_stack_len + *expr_j;
                        // Resolve reference of computed expression.
                        let id = if let &Variable::Ref(ref_id) = &stack[id] {
                                ref_id
                            } else {
                                id
                            };
                        match &mut stack[id] {
                            &mut Variable::Text(ref id) => {
                                *expr_j += 1;
                                id.clone()
                            }
                            _ => return Err(module.error_fnindex(prop.source_range(),
                                &format!("{}\nExpected string",
                                    stack_trace(call_stack)),
                                    call_stack.last().unwrap().index))
                        }
                    }
                    &Id::F64(range, _) => return Err(module.error_fnindex(range,
                        &format!("{}\nExpected string",
                            stack_trace(call_stack)),
                            call_stack.last().unwrap().index))
                };
                let v = match Arc::make_mut(obj).entry(id.clone()) {
                    Entry::Vacant(vac) => {
                        if insert && last {
                            // Insert a key to overwrite with new value.
                            vac.insert(Variable::Return)
                        } else {
                            return Err(module.error_fnindex(prop.source_range(),
                                &format!("{}\nObject has no key `{}`",
                                    stack_trace(call_stack), id),
                                    call_stack.last().unwrap().index));
                        }
                    }
                    Entry::Occupied(v) => v.into_mut()
                };
                // Resolve reference.
                if let &mut Variable::Ref(id) = v {
                    // Do not resolve if last, because references should be
                    // copy-on-write.
                    if last {
                        Ok(v)
                    } else {
                        Ok(&mut stack[id])
                    }
                } else {
                    Ok(v)
                }
            }
            Variable::Array(ref mut arr) => {
                let id = match prop {
                    &Id::F64(_, id) => id,
                    &Id::Expression(_) => {
                        let id = start_stack_len + *expr_j;
                        // Resolve reference of computed expression.
                        let id = if let &Variable::Ref(ref_id) = &stack[id] {
                                ref_id
                            } else {
                                id
                            };
                        match &mut stack[id] {
                            &mut Variable::F64(id, _) => {
                                *expr_j += 1;
                                id
                            }
                            _ => return Err(module.error_fnindex(prop.source_range(),
                                            &format!("{}\nExpected number",
                                                stack_trace(call_stack)),
                                                call_stack.last().unwrap().index))
                        }
                    }
                    &Id::String(range, _) => return Err(module.error_fnindex(range,
                        &format!("{}\nExpected number",
                            stack_trace(call_stack)),
                            call_stack.last().unwrap().index))
                };
                let v = match Arc::make_mut(arr).get_mut(id as usize) {
                    None => return Err(module.error_fnindex(prop.source_range(),
                                       &format!("{}\nOut of bounds `{}`",
                                                stack_trace(call_stack), id),
                                                call_stack.last().unwrap().index)),
                    Some(x) => x
                };
                // Resolve reference.
                if let &mut Variable::Ref(id) = v {
                    // Do not resolve if last, because references should be
                    // copy-on-write.
                    if last {
                        Ok(v)
                    } else {
                        Ok(&mut stack[id])
                    }
                } else {
                    Ok(v)
                }
            }
            _ => return Err(module.error_fnindex(prop.source_range(),
                            &format!("{}\nLook up requires object or array",
                            stack_trace(call_stack)),
                            call_stack.last().unwrap().index))
        }
    }
}

impl Runtime {
    /// Creates a new Runtime.
    pub fn new() -> Runtime {
        use rand::FromEntropy;

        Runtime {
            stack: vec![],
            call_stack: vec![],
            local_stack: vec![],
            current_stack: vec![],
            ret: Arc::new("return".into()),
            rng: rand::rngs::StdRng::from_entropy(),
        }
    }

    /// Pops variable from stack.
    pub fn pop<T: embed::PopVariable>(&mut self) -> Result<T, String> {
        let v = self.stack.pop().unwrap_or_else(|| panic!(TINVOTS));
        T::pop_var(self, self.resolve(&v))
    }

    /// Pops 4D vector from stack.
    pub fn pop_vec4<T: embed::ConvertVec4>(&mut self) -> Result<T, String> {
        let v = self.stack.pop().unwrap_or_else(|| panic!(TINVOTS));
        match self.resolve(&v) {
            &Variable::Vec4(val) => Ok(T::from(val)),
            x => Err(self.expected(x, "vec4"))
        }
    }

    /// Pops 4D matrix from stack.
    pub fn pop_mat4<T: embed::ConvertMat4>(&mut self) -> Result<T, String> {
        let v = self.stack.pop().unwrap_or_else(|| panic!(TINVOTS));
        match self.resolve(&v) {
            &Variable::Mat4(ref val) => Ok(T::from(**val)),
            x => Err(self.expected(x, "mat4"))
        }
    }

    /// Gets variable.
    pub fn var<T: embed::PopVariable>(&self, var: &Variable) -> Result<T, String> {
        T::pop_var(self, self.resolve(&var))
    }

    /// Gets 4D vector.
    pub fn var_vec4<T: embed::ConvertVec4>(&self, var: &Variable) -> Result<T, String> {
        match self.resolve(&var) {
            &Variable::Vec4(val) => Ok(T::from(val)),
            x => Err(self.expected(x, "vec4"))
        }
    }

    /// Gets 4D matrix.
    pub fn var_mat4<T: embed::ConvertMat4>(&self, var: &Variable) -> Result<T, String> {
        match self.resolve(&var) {
            &Variable::Mat4(ref val) => Ok(T::from(**val)),
            x => Err(self.expected(x, "mat4"))
        }
    }

    /// Push value to stack.
    pub fn push<T: embed::PushVariable>(&mut self, val: T) {
        self.stack.push(val.push_var())
    }

    /// Push Vec4 to stack.
    pub fn push_vec4<T: embed::ConvertVec4>(&mut self, val: T) {
        self.stack.push(Variable::Vec4(val.to()))
    }

    /// Push Mat4 to stack.
    pub fn push_mat4<T: embed::ConvertMat4>(&mut self, val: T) {
        self.stack.push(Variable::Mat4(Box::new(val.to())))
    }

    /// Pushes Rust object to stack.
    pub fn push_rust<T: 'static>(&mut self, val: T) {
        use std::sync::Mutex;
        use RustObject;
        self.stack.push(Variable::RustObject(Arc::new(Mutex::new(val)) as RustObject))
    }

    /// Generates error message that a certain type was expected.
    pub fn expected(&self, var: &Variable, ty: &str) -> String {
        let found_ty = var.typeof_var();
        format!("{}\nExpected `{}`, found `{}`", self.stack_trace(), ty, found_ty)
    }

    /// Resolves a variable reference if any, getting a pointer to the variable on the stack.
    #[inline(always)]
    pub fn resolve<'a>(&'a self, var: &'a Variable) -> &'a Variable {
        resolve(&self.stack, var)
    }

    #[inline(always)]
    fn push_fn(
        &mut self,
        name: Arc<String>,
        index: usize,
        file: Option<Arc<String>>,
        st: usize,
        lc: usize,
        cu: usize,
    ) {
        self.call_stack.push(Call {
            fn_name: name,
            index: index,
            file: file,
            stack_len: st,
            local_len: lc,
            current_len: cu,
        });
    }
    fn pop_fn(&mut self, name: Arc<String>) {
        match self.call_stack.pop() {
            None => panic!("Did not call `{}`", name),
            Some(Call { fn_name, stack_len: st, local_len: lc, current_len: cu, .. }) => {
                if name != fn_name {
                    panic!("Calling `{}`, did not call `{}`", fn_name, name);
                }
                self.stack.truncate(st);
                self.local_stack.truncate(lc);
                self.current_stack.truncate(cu);
            }
        }
    }

    pub(crate) fn expression(
        &mut self,
        expr: &ast::Expression,
        side: Side,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        use ast::Expression::*;

        match *expr {
            Link(ref link) => self.link(link, module),
            Object(ref obj) => self.object(obj, module),
            Array(ref arr) => self.array(arr, module),
            ArrayFill(ref array_fill) => self.array_fill(array_fill, module),
            Block(ref block) => self.block(block, module),
            Return(ref ret) => {
                let x = match try!(self.expression(ret, Side::Right, module)) {
                    (Some(x), Flow::Continue) => x,
                    (x, Flow::Return) => { return Ok((x, Flow::Return)); }
                    _ => return Err(module.error(expr.source_range(),
                                    &format!("{}\nExpected something",
                                        self.stack_trace()), self))
                };
                Ok((Some(x), Flow::Return))
            }
            ReturnVoid(_) => Ok((None, Flow::Return)),
            Break(ref b) => Ok((None, Flow::Break(b.label.clone()))),
            Continue(ref b) => Ok((None, Flow::ContinueLoop(b.label.clone()))),
            Go(ref go) => self.go(go, module),
            Call(ref call) => {
                let loader = false;
                self.call_internal(call, loader, module)
            }
            Item(ref item) => self.item(item, side, module),
            Norm(ref norm) => self.norm(norm, side, module),
            UnOp(ref unop) => self.unop(unop, side, module),
            BinOp(ref binop) => self.binop(binop, side, module),
            Assign(ref assign) => self.assign(assign.op, &assign.left, &assign.right, module),
            Vec4(ref vec4) => self.vec4(vec4, side, module),
            Mat4(ref mat4) => self.mat4(mat4, side, module),
            For(ref for_expr) => self.for_expr(for_expr, module),
            ForN(ref for_n_expr) => self.for_n_expr(for_n_expr, module),
            ForIn(ref for_in_expr) => self.for_in_expr(for_in_expr, module),
            Sum(ref for_n_expr) => self.sum_n_expr(for_n_expr, module),
            SumIn(ref sum_in_expr) => self.sum_in_expr(sum_in_expr, module),
            SumVec4(ref for_n_expr) => self.sum_vec4_n_expr(for_n_expr, module),
            Prod(ref for_n_expr) => self.prod_n_expr(for_n_expr, module),
            ProdIn(ref for_in_expr) => self.prod_in_expr(for_in_expr, module),
            ProdVec4(ref for_n_expr) => self.prod_vec4_n_expr(for_n_expr, module),
            Min(ref for_n_expr) => self.min_n_expr(for_n_expr, module),
            MinIn(ref for_in_expr) => self.min_in_expr(for_in_expr, module),
            Max(ref for_n_expr) => self.max_n_expr(for_n_expr, module),
            MaxIn(ref for_in_expr) => self.max_in_expr(for_in_expr, module),
            Sift(ref for_n_expr) => self.sift_n_expr(for_n_expr, module),
            SiftIn(ref for_in_expr) => self.sift_in_expr(for_in_expr, module),
            Any(ref for_n_expr) => self.any_n_expr(for_n_expr, module),
            AnyIn(ref for_in_expr) => self.any_in_expr(for_in_expr, module),
            All(ref for_n_expr) => self.all_n_expr(for_n_expr, module),
            AllIn(ref for_in_expr) => self.all_in_expr(for_in_expr, module),
            LinkFor(ref for_n_expr) => self.link_for_n_expr(for_n_expr, module),
            LinkIn(ref for_in_expr) => self.link_for_in_expr(for_in_expr, module),
            If(ref if_expr) => self.if_expr(if_expr, module),
            Compare(ref compare) => self.compare(compare, module),
            Variable(ref range_var) => Ok((Some(range_var.1.clone()), Flow::Continue)),
            Try(ref expr) => self.try(expr, side, module),
            Swizzle(ref sw) => {
                let flow = try!(self.swizzle(sw, module));
                Ok((None, flow))
            }
            Closure(ref closure) => self.closure(closure, module),
            CallClosure(ref call) => self.call_closure(call, module),
            Grab(ref expr) => Err(module.error(expr.source_range,
                    &format!("{}\n`grab` expressions must be inside a closure",
                        self.stack_trace()), self)),
            TryExpr(ref try_expr) => self.try_expr(try_expr, module),
            In(ref in_expr) => self.in_expr(in_expr, module),
        }
    }

    fn in_expr(&mut self, in_expr: &ast::In, module: &Arc<Module>)
    -> Result<(Option<Variable>, Flow), String> {
        use std::sync::mpsc::channel;
        use std::sync::Mutex;
        use std::sync::atomic::Ordering;

        match in_expr.f_index.get() {
            FnIndex::Loaded(f_index) => {
                let relative = self.call_stack.last().map(|c| c.index).unwrap_or(0);
                let new_index = (f_index + relative as isize) as usize;
                let f = &module.functions[new_index];
                let (tx, rx) = channel();
                // Guard the change of flag to avoid data race.
                let mut guard = f.senders.1.lock().unwrap();
                guard.push(tx);
                f.senders.0.store(true, Ordering::Relaxed);
                drop(guard);
                Ok((Some(::Variable::In(Arc::new(Mutex::new(rx)))), Flow::Continue))
            }
            _ => Err(module.error(in_expr.source_range,
                    &format!("{}\nExpected loaded function",
                        self.stack_trace()), self)),
        }
    }

    fn try_expr(&mut self, try_expr: &ast::TryExpr, module: &Arc<Module>)
    -> Result<(Option<Variable>, Flow), String> {
        use Error;

        let cs = self.call_stack.len();
        let st = self.stack.len();
        let lc = self.local_stack.len();
        let cu = self.current_stack.len();
        match self.expression(&try_expr.expr, Side::Right, module) {
            Ok((Some(x), Flow::Continue)) => Ok((
                Some(Variable::Result(Ok(Box::new(x)))),
                Flow::Continue
            )),
            Ok((None, Flow::Continue)) => Err(module.error(try_expr.source_range,
                &format!("{}\nExpected something", self.stack_trace()), self)),
            Ok((x, flow)) => Ok((x, flow)),
            Err(err) => {
                self.call_stack.truncate(cs);
                self.stack.truncate(st);
                self.local_stack.truncate(lc);
                self.current_stack.truncate(cu);
                Ok((
                    Some(Variable::Result(Err(Box::new(Error {
                        message: Variable::Text(Arc::new(err)),
                        trace: vec![],
                    }
                    )))),
                    Flow::Continue
                ))
            }
        }
    }

    fn closure(&mut self, closure: &ast::Closure, module: &Arc<Module>)
    -> Result<(Option<Variable>, Flow), String> {
        use grab::{self, Grabbed};
        use ClosureEnvironment;

        // Create closure.
        let relative = self.call_stack.last().map(|c| c.index).unwrap_or(0);
        // Evaluate `grab` expressions and generate new AST.
        let new_expr = match try!(grab::grab_expr(1, self, &closure.expr, Side::Right, module)) {
            (Grabbed::Expression(x), Flow::Continue) => x,
            (Grabbed::Variable(x), Flow::Return) => { return Ok((x, Flow::Return)); }
            _ => return Err(module.error(closure.expr.source_range(),
                            &format!("{}\nExpected something",
                                self.stack_trace()), self))
        };

        Ok((Some(::Variable::Closure(Arc::new(ast::Closure {
            currents: closure.currents.clone(),
            args: closure.args.clone(),
            source_range: closure.source_range.clone(),
            ret: closure.ret.clone(),
            file: closure.file.clone(),
            source: closure.source.clone(),
            expr: new_expr,
        }), Box::new(ClosureEnvironment {
            module: module.clone(),
            relative: relative
        }))), Flow::Continue))
    }

    fn try_msg(v: &Variable) -> Option<Result<Box<Variable>, Box<::Error>>> {
        use Error;

        Some(match v {
            &Variable::Result(ref res) => res.clone(),
            &Variable::Option(ref opt) => {
                match opt {
                    &Some(ref some) => Ok(some.clone()),
                    &None => Err(Box::new(Error {
                        message: Variable::Text(Arc::new(
                            "Expected `some(_)`, found `none()`"
                            .into())),
                        trace: vec![]
                    }))
                }
            }
            &Variable::Bool(true, None) => {
                Err(Box::new(Error {
                    message: Variable::Text(Arc::new(
                        "This does not make sense, perhaps an array is empty?"
                        .into())),
                    trace: vec![]
                }))
            }
            &Variable::Bool(false, _) => {
                Err(Box::new(Error {
                    message: Variable::Text(Arc::new(
                        "Must be `true` to have meaning, try add or remove `!`"
                        .into())),
                    trace: vec![]
                }))
            }
            &Variable::Bool(true, ref sec) => {
                match sec {
                    &None => Err(Box::new(Error {
                        message: Variable::Text(Arc::new(
                            "Expected `some(_)`, found `none()`"
                            .into())),
                        trace: vec![]
                    })),
                    &Some(_) => {
                        Ok(Box::new(Variable::Bool(true, sec.clone())))
                    }
                }
            }
            &Variable::F64(val, ref sec) => {
                if val.is_nan() {
                    Err(Box::new(Error {
                        message: Variable::Text(Arc::new(
                            "Expected number, found `NaN`"
                            .into())),
                        trace: vec![]
                    }))
                } else if sec.is_none() {
                    Err(Box::new(Error {
                        message: Variable::Text(Arc::new(
                            "This does not make sense, perhaps an array is empty?"
                            .into())),
                        trace: vec![]
                    }))
                } else {
                    Ok(Box::new(Variable::F64(val, sec.clone())))
                }
            }
            _ => return None
        })
    }

    fn try(
        &mut self,
        expr: &ast::Expression,
        side: Side,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        let v = match try!(self.expression(expr, side, module)) {
            (Some(x), Flow::Continue) => x,
            (x, Flow::Return) => { return Ok((x, Flow::Return)); }
            _ => return Err(module.error(expr.source_range(),
                            &format!("{}\nExpected something",
                                self.stack_trace()), self))
        };
        let v = match Runtime::try_msg(self.resolve(&v)) {
            Some(v) => v,
            None => {
                return Err(module.error(expr.source_range(),
                    &format!("{}\nExpected `ok(_)`, `err(_)`, `bool`, `f64`",
                        self.stack_trace()), self));
            }
        };
        match v {
            Ok(ok) => {
                Ok((Some(*ok), Flow::Continue))
            }
            Err(mut err) => {
                let call = self.call_stack.last().unwrap();
                if call.stack_len == 0 {
                    return Err(module.error(expr.source_range(),
                        &format!("{}\nRequires `->` on function `{}`",
                        self.stack_trace(),
                        &call.fn_name), self));
                }
                if let Variable::Return = self.stack[call.stack_len - 1] {}
                else {
                    return Err(module.error(expr.source_range(),
                        &format!("{}\nRequires `->` on function `{}`",
                        self.stack_trace(),
                        &call.fn_name), self));
                }
                let file = match call.file {
                    None => "".into(),
                    Some(ref f) => format!(" ({})", f)
                };
                err.trace.push(module.error(expr.source_range(),
                    &format!("In function `{}`{}",
                    &call.fn_name, file), self));
                Ok((Some(Variable::Result(Err(err))), Flow::Return))
            }
        }
    }

    /// Run `main` function in a module.
    pub fn run(&mut self, module: &Arc<Module>) -> Result<(), String> {
        use std::cell::Cell;

        let name: Arc<String> = Arc::new("main".into());
        let call = ast::Call {
            alias: None,
            name: name.clone(),
            f_index: Cell::new(module.find_function(&name, 0)),
            args: vec![],
            custom_source: None,
            source_range: Range::empty(0),
        };
        match call.f_index.get() {
            FnIndex::Loaded(f_index) => {
                let f = &module.functions[f_index as usize];
                if f.args.len() != 0 {
                    return Err(module.error(f.args[0].source_range,
                               "`main` should not have arguments", self))
                }
                let loader = false;
                try!(self.call_internal(&call, loader, &module));
                Ok(())
            }
            _ => return Err(module.error(call.source_range,
                               "Could not find function `main`", self))
        }
    }

    fn block(
        &mut self,
        block: &ast::Block,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        let mut expect = None;
        let st = self.stack.len();
        let lc = self.local_stack.len();
        let cu = self.current_stack.len();
        for e in &block.expressions {
            expect = match try!(self.expression(e, Side::Right, module)) {
                (x, Flow::Continue) => x,
                x => {
                    self.stack.truncate(st);
                    self.local_stack.truncate(lc);
                    self.current_stack.truncate(cu);
                    return Ok(x);
                }
            }
        }

        self.stack.truncate(st);
        self.local_stack.truncate(lc);
        self.current_stack.truncate(cu);
        Ok((expect, Flow::Continue))
    }

    /// Start a new thread and return the handle.
    pub fn go(&mut self, go: &ast::Go, module: &Arc<Module>) -> Result<(Option<Variable>, Flow), String> {
        use std::thread::{self, JoinHandle};
        use std::cell::Cell;
        use Thread;

        let n = go.call.args.len();
        let mut stack = vec![];
        let relative = self.call_stack.last().map(|c| c.index).unwrap();
        let mut fake_call = ast::Call {
            alias: go.call.alias.clone(),
            name: go.call.name.clone(),
            f_index: Cell::new(module.find_function(&go.call.name, relative)),
            args: Vec::with_capacity(n),
            custom_source: None,
            source_range: go.call.source_range,
        };
        // Evaluate the arguments and put a deep clone on the new stack.
        // This prevents the arguments from containing any reference to other variables.
        for (i, arg) in go.call.args.iter().enumerate() {
            let v = match try!(self.expression(arg, Side::Right, module)) {
                (Some(x), Flow::Continue) => x,
                (x, Flow::Return) => { return Ok((x, Flow::Return)); }
                _ => return Err(module.error(arg.source_range(),
                                &format!("{}\nExpected something. \
                                Expression did not return a value.",
                                self.stack_trace()), self))
            };
            stack.push(v.deep_clone(&self.stack));
            fake_call.args.push(ast::Expression::Variable(Box::new((
                go.call.args[i].source_range(), Variable::Ref(n-i-1)))));
        }
        stack.reverse();

        let last_call = self.call_stack.last().unwrap();
        let new_rt = Runtime {
            stack: stack,
            local_stack: vec![],
            current_stack: vec![],
            // Add last call because of loaded functions
            // use relative index to the function it is calling from.
            call_stack: vec![Call {
                fn_name: last_call.fn_name.clone(),
                index: last_call.index,
                file: last_call.file.clone(),
                stack_len: 0,
                local_len: 0,
                current_len: 0,
            }],
            rng: self.rng.clone(),
            ret: self.ret.clone(),
        };
        let new_module = module.clone();
        let handle: JoinHandle<Result<Variable, String>> = thread::spawn(move || {
            let mut new_rt = new_rt;
            let new_module = new_module;
            let fake_call = fake_call;
            let loader = false;
            Ok(match new_rt.call_internal(&fake_call, loader, &new_module) {
                Err(err) => return Err(err),
                Ok((None, _)) => {
                    new_rt.stack.pop().expect(TINVOTS)
                }
                Ok((Some(x), _)) => x,
            }.deep_clone(&new_rt.stack))
        });
        Ok((Some(Variable::Thread(Thread::new(handle))), Flow::Continue))
    }

    /// Call closure.
    pub fn call_closure(
        &mut self,
        call: &ast::CallClosure,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        // Find item.
        let item = match try!(self.item(&call.item, Side::Right, module)) {
            (Some(x), Flow::Continue) => x,
            (x, Flow::Return) => { return Ok((x, Flow::Return)); }
            _ => return Err(module.error(call.item.source_range,
                            &format!("{}\nExpected something. \
                            Check that item returns a value.",
                            self.stack_trace()), self))
        };

        let (f, env) = match self.resolve(&item) {
            &Variable::Closure(ref f, ref env) => (f.clone(), env.clone()),
            x => return Err(module.error(call.source_range,
                    &self.expected(x, "closure"), self))
        };

        if call.arg_len() != f.args.len() {
            return Err(module.error(call.source_range,
                &format!("{}\nExpected {} arguments but found {}",
                self.stack_trace(),
                f.args.len(),
                call.arg_len()), self));
        }
        // Arguments must be computed.
        if f.returns() {
            // Add return value before arguments on the stack.
            // The stack value should remain, but the local should not.
            self.stack.push(Variable::Return);
        }
        let st = self.stack.len();
        let lc = self.local_stack.len();
        let cu = self.current_stack.len();
        for arg in &call.args {
            match try!(self.expression(arg, Side::Right, module)) {
                (Some(x), Flow::Continue) => self.stack.push(x),
                (None, Flow::Continue) => {}
                (x, Flow::Return) => { return Ok((x, Flow::Return)); }
                _ => return Err(module.error(arg.source_range(),
                                &format!("{}\nExpected something. \
                                Check that expression returns a value.",
                                self.stack_trace()), self))
            };
        }

        // Look for variable in current stack.
        if f.currents.len() > 0 {
            for current in &f.currents {
                let mut res = None;
                for &(ref cname, ind) in self.current_stack.iter().rev() {
                    if cname == &current.name {
                        res = Some(ind);
                        break;
                    }
                }
                if let Some(ind) = res {
                    self.local_stack.push((current.name.clone(), self.stack.len()));
                    self.stack.push(Variable::Ref(ind));
                } else {
                    return Err(module.error(call.source_range, &format!(
                        "{}\nCould not find current variable `{}`",
                            self.stack_trace(), current.name), self));
                }
            }
        }

        self.push_fn(call.item.name.clone(), env.relative, Some(f.file.clone()), st, lc, cu);
        if f.returns() {
            self.local_stack.push((self.ret.clone(), st - 1));
        }
        for (i, arg) in f.args.iter().enumerate() {
            // Do not resolve locals to keep fixed length from end of stack.
            self.local_stack.push((arg.name.clone(), st + i));
        }
        let (x, flow) = try!(self.expression(&f.expr, Side::Right, &env.module));
        match flow {
            Flow::Break(None) =>
                return Err(module.error(call.source_range,
                           &format!("{}\nCan not break from function",
                                self.stack_trace()), self)),
            Flow::ContinueLoop(None) =>
                return Err(module.error(call.source_range,
                           &format!("{}\nCan not continue from function",
                                self.stack_trace()), self)),
            Flow::Break(Some(ref label)) =>
                return Err(module.error(call.source_range,
                    &format!("{}\nThere is no loop labeled `{}`",
                             self.stack_trace(), label), self)),
            Flow::ContinueLoop(Some(ref label)) =>
                return Err(module.error(call.source_range,
                    &format!("{}\nThere is no loop labeled `{}`",
                            self.stack_trace(), label), self)),
            _ => {}
        }
        self.pop_fn(call.item.name.clone());
        match (f.returns(), x) {
            (true, None) => {
                match self.stack.pop().expect(TINVOTS) {
                    Variable::Return => {
                        return Err(module.error(
                            call.source_range, &format!(
                            "{}\nFunction `{}` did not return a value",
                            self.stack_trace(),
                            call.item.name), self))
                    }
                    x => {
                        // This happens when return is only
                        // assigned to `return = x`.
                        return Ok((Some(x), Flow::Continue))
                    }
                };
            }
            (false, Some(_)) => {
                return Err(module.error(call.source_range,
                    &format!(
                        "{}\nFunction `{}` should not return a value",
                        self.stack_trace(),
                        call.item.name), self))
            }
            (true, Some(Variable::Return)) => {
                // TODO: Could return the last value on the stack.
                //       Requires .pop_fn delayed after.
                return Err(module.error(call.source_range,
                    &format!(
                    "{}\nFunction `{}` did not return a value. \
                    Did you forget a `return`?",
                        self.stack_trace(),
                        call.item.name), self))
            }
            (returns, b) => {
                if returns { self.stack.pop(); }
                return Ok((b, Flow::Continue))
            }
        }
    }

    /// Called from the outside, e.g. a loader script by `call` or `call_ret` intrinsic.
    pub fn call(
        &mut self,
        call: &ast::Call,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        self.call_internal(call, true, module)
    }

    /// Used internally because loaded functions are resolved
    /// relative to the caller, which stores its index on the
    /// call stack.
    ///
    /// The `loader` flag is set to `true` when called from the outside.
    fn call_internal(
        &mut self,
        call: &ast::Call,
        loader: bool,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        use FnExternalRef;

        match call.f_index.get() {
            FnIndex::Intrinsic(index) => {
                intrinsics::call_standard(self, index, call, module)
            }
            FnIndex::ExternalVoid(FnExternalRef(f)) => {
                for arg in &call.args {
                    match try!(self.expression(arg, Side::Right, module)) {
                        (Some(x), Flow::Continue) => self.stack.push(x),
                        (x, Flow::Return) => { return Ok((x, Flow::Return)); }
                        _ => return Err(module.error(arg.source_range(),
                                        &format!("{}\nExpected something. \
                                        Expression did not return a value.",
                                        self.stack_trace()), self))
                    };
                }
                try!((f)(self).map_err(|err|
                    module.error(call.source_range, &err, self)));
                return Ok((None, Flow::Continue));
            }
            FnIndex::ExternalReturn(FnExternalRef(f)) => {
                for arg in &call.args {
                    match try!(self.expression(arg, Side::Right, module)) {
                        (Some(x), Flow::Continue) => self.stack.push(x),
                        (x, Flow::Return) => { return Ok((x, Flow::Return)); }
                        _ => return Err(module.error(arg.source_range(),
                                        &format!("{}\nExpected something. \
                                        Expression did not return a value.",
                                        self.stack_trace()), self))
                    };
                }
                try!((f)(self).map_err(|err|
                    module.error(call.source_range, &err, self)));
                return Ok((Some(self.stack.pop().expect(TINVOTS)), Flow::Continue));
            }
            FnIndex::Loaded(f_index) => {
                use std::sync::atomic::Ordering;

                let relative = if loader {0} else {
                    self.call_stack.last().map(|c| c.index).unwrap_or(0)
                };
                let new_index = (f_index + relative as isize) as usize;
                let f = &module.functions[new_index];
                if call.arg_len() != f.args.len() {
                    return Err(module.error(call.source_range,
                        &format!("{}\nExpected {} arguments but found {}",
                        self.stack_trace(),
                        f.args.len(),
                        call.arg_len()), self));
                }
                // Arguments must be computed.
                if f.returns() {
                    // Add return value before arguments on the stack.
                    // The stack value should remain, but the local should not.
                    self.stack.push(Variable::Return);
                }
                let st = self.stack.len();
                let lc = self.local_stack.len();
                let cu = self.current_stack.len();

                for arg in &call.args {
                    match try!(self.expression(arg, Side::Right, module)) {
                        (Some(x), Flow::Continue) => self.stack.push(x),
                        (None, Flow::Continue) => {}
                        (x, Flow::Return) => { return Ok((x, Flow::Return)); }
                        _ => return Err(module.error(arg.source_range(),
                                        &format!("{}\nExpected something. \
                                        Check that expression returns a value.",
                                        self.stack_trace()), self))
                    };
                }

                // Look for variable in current stack.
                if f.currents.len() > 0 {
                    for current in &f.currents {
                        let mut res = None;
                        for &(ref cname, ind) in self.current_stack.iter().rev() {
                            if cname == &current.name {
                                res = Some(ind);
                                break;
                            }
                        }
                        if let Some(ind) = res {
                            self.local_stack.push((current.name.clone(), self.stack.len()));
                            self.stack.push(Variable::Ref(ind));
                        } else {
                            return Err(module.error(call.source_range, &format!(
                                "{}\nCould not find current variable `{}`",
                                    self.stack_trace(), current.name), self));
                        }
                    }
                }

                // Send arguments to senders.
                if f.senders.0.load(Ordering::Relaxed) {
                    let n = self.stack.len();
                    let mut msg = Vec::with_capacity(n - st);
                    for i in st..n {
                        msg.push(self.stack[i].deep_clone(&self.stack));
                    }
                    let msg = Arc::new(msg);
                    // Uses smart swapping of channels to put the closed ones at the end.
                    let ref mut channels = f.senders.1.lock().unwrap();
                    let mut open = channels.len();
                    for i in (0..channels.len()).rev() {
                        match channels[i].send(Variable::Array(msg.clone())) {
                            Ok(_) => {}
                            Err(_) => {
                                open -= 1;
                                channels.swap(i, open);
                            }
                        }
                    }
                    channels.truncate(open);
                    if channels.len() == 0 {
                        // Change of flag is guarded by the mutex.
                        f.senders.0.store(false, Ordering::Relaxed);
                    }
                    drop(channels);
                }

                self.push_fn(call.name.clone(), new_index, Some(f.file.clone()), st, lc, cu);
                if f.returns() {
                    self.local_stack.push((self.ret.clone(), st - 1));
                }
                for (i, arg) in f.args.iter().enumerate() {
                    // Do not resolve locals to keep fixed length from end of stack.
                    self.local_stack.push((arg.name.clone(), st + i));
                }
                let (x, flow) = try!(self.block(&f.block, module));
                match flow {
                    Flow::Break(None) =>
                        return Err(module.error(call.source_range,
                                   &format!("{}\nCan not break from function",
                                        self.stack_trace()), self)),
                    Flow::ContinueLoop(None) =>
                        return Err(module.error(call.source_range,
                                   &format!("{}\nCan not continue from function",
                                        self.stack_trace()), self)),
                    Flow::Break(Some(ref label)) =>
                        return Err(module.error(call.source_range,
                            &format!("{}\nThere is no loop labeled `{}`",
                                     self.stack_trace(), label), self)),
                    Flow::ContinueLoop(Some(ref label)) =>
                        return Err(module.error(call.source_range,
                            &format!("{}\nThere is no loop labeled `{}`",
                                    self.stack_trace(), label), self)),
                    _ => {}
                }
                self.pop_fn(call.name.clone());
                match (f.returns(), x) {
                    (true, None) => {
                        match self.stack.pop().expect(TINVOTS) {
                            Variable::Return => {
                                let source = call.custom_source.as_ref().unwrap_or(
                                    &module.functions[
                                        self.call_stack.last().unwrap().index
                                    ].source
                                );
                                return Err(module.error_source(
                                call.source_range, &format!(
                                "{}\nFunction `{}` did not return a value",
                                self.stack_trace(),
                                f.name), source))
                            }
                            x => {
                                // This happens when return is only
                                // assigned to `return = x`.
                                return Ok((Some(x), Flow::Continue))
                            }
                        };
                    }
                    (false, Some(_)) => {
                        let source = call.custom_source.as_ref().unwrap_or(
                            &module.functions[self.call_stack.last().unwrap().index].source
                        );
                        return Err(module.error_source(call.source_range,
                            &format!(
                                "{}\nFunction `{}` should not return a value",
                                self.stack_trace(),
                                f.name), source))
                    }
                    (true, Some(Variable::Return)) => {
                        // TODO: Could return the last value on the stack.
                        //       Requires .pop_fn delayed after.
                        let source = call.custom_source.as_ref().unwrap_or(
                            &module.functions[self.call_stack.last().unwrap().index].source
                        );
                        return Err(module.error_source(call.source_range,
                            &format!(
                            "{}\nFunction `{}` did not return a value. \
                            Did you forget a `return`?",
                                self.stack_trace(),
                                f.name), source))
                    }
                    (returns, b) => {
                        if returns { self.stack.pop(); }
                        return Ok((b, Flow::Continue))
                    }
                }
            }
            FnIndex::None => {
                return Err(module.error(call.source_range,
                    &format!("{}\nUnknown function `{}`", self.stack_trace(), call.name), self))
            }
        }
    }

    /// Calls function by name.
    pub fn call_str(
        &mut self,
        function: &str,
        args: &[Variable],
        module: &Arc<Module>
    ) -> Result<(), String> {
        use std::cell::Cell;

        let name: Arc<String> = Arc::new(function.into());
        match module.find_function(&name, 0) {
            FnIndex::Loaded(f_index) => {
                let call = ast::Call {
                    alias: None,
                    name: name.clone(),
                    f_index: Cell::new(FnIndex::Loaded(f_index)),
                    args: args.iter()
                            .map(|arg| ast::Expression::Variable(Box::new((
                                       Range::empty(0), arg.clone()))))
                            .collect(),
                    custom_source: None,
                    source_range: Range::empty(0),
                };
                try!(self.call(&call, &module));
                Ok(())
            }
            _ => return Err(format!("Could not find function `{}`",function))
        }
    }

    /// Call function by name, returning a value.
    pub fn call_str_ret(
        &mut self,
        function: &str,
        args: &[Variable],
        module: &Arc<Module>
    ) -> Result<Variable, String> {
        use std::cell::Cell;

        let name: Arc<String> = Arc::new(function.into());
        match module.find_function(&name, 0) {
            FnIndex::Loaded(f_index) => {
                let call = ast::Call {
                    alias: None,
                    name: name.clone(),
                    f_index: Cell::new(FnIndex::Loaded(f_index)),
                    args: args.iter()
                            .map(|arg| ast::Expression::Variable(Box::new((
                                       Range::empty(0), arg.clone()))))
                            .collect(),
                    custom_source: None,
                    source_range: Range::empty(0),
                };
                match self.call(&call, &module) {
                    Ok((Some(val), Flow::Continue)) => Ok(val),
                    Err(err) => Err(err),
                    _ => return Err(module.error(call.source_range,
                                    &format!("{}\nExpected something",
                                        self.stack_trace()), self))
                }
            }
            _ => return Err(format!("Could not find function `{}`",function))
        }
    }

    fn swizzle(&mut self, sw: &ast::Swizzle, module: &Arc<Module>) -> Result<Flow, String> {
        let v = match try!(self.expression(&sw.expr, Side::Right, module)) {
            (Some(x), Flow::Continue) => x,
            (_, Flow::Return) => { return Ok(Flow::Return); }
            _ => return Err(module.error(sw.expr.source_range(),
                            &format!("{}\nExpected something",
                                self.stack_trace()), self))
        };
        let v = match self.resolve(&v) {
            &Variable::Vec4(v) => v,
            x => return Err(module.error(sw.source_range,
                    &self.expected(x, "vec4"), self))
        };
        self.stack.push(Variable::f64(v[sw.sw0] as f64));
        self.stack.push(Variable::f64(v[sw.sw1] as f64));
        if let Some(ind) = sw.sw2 {
            self.stack.push(Variable::f64(v[ind] as f64));
        }
        if let Some(ind) = sw.sw3 {
            self.stack.push(Variable::f64(v[ind] as f64));
        }
        Ok(Flow::Continue)
    }

    fn link(
        &mut self,
        link: &ast::Link,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        use Link;

        Ok((Some(if link.items.len() == 0 {
            Variable::Link(Box::new(Link::new()))
        } else {
            let st = self.stack.len();
            let lc = self.local_stack.len();
            let cu = self.current_stack.len();
            let mut new_link = Link::new();
            for item in &link.items {
                let v = match try!(self.expression(item, Side::Right, module)) {
                    (Some(x), Flow::Continue) => x,
                    (None, Flow::Continue) => continue,
                    (res, flow) => { return Ok((res, flow)); }
                };
                match new_link.push(self.resolve(&v)) {
                    Err(err) => {
                        return Err(module.error(item.source_range(),
                            &format!("{}\n{}", self.stack_trace(),
                            err), self))
                    }
                    Ok(()) => {}
                }
            }
            self.stack.truncate(st);
            self.local_stack.truncate(lc);
            self.current_stack.truncate(cu);
            Variable::Link(Box::new(new_link))
        }), Flow::Continue))
    }

    fn object(
        &mut self,
        obj: &ast::Object,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        let mut object: HashMap<_, _> = HashMap::new();
        for &(ref key, ref expr) in &obj.key_values {
            let x = match try!(self.expression(expr, Side::Right, module)) {
                (Some(x), Flow::Continue) => x,
                (x, Flow::Return) => { return Ok((x, Flow::Return)); }
                _ => return Err(module.error(expr.source_range(),
                                &format!("{}\nExpected something",
                                    self.stack_trace()), self))
            };
            match object.insert(key.clone(), x) {
                None => {}
                Some(_) => return Err(module.error(expr.source_range(),
                    &format!("{}\nDuplicate key in object `{}`",
                        self.stack_trace(), key), self))
            }
        }
        Ok((Some(Variable::Object(Arc::new(object))), Flow::Continue))
    }

    fn array(
        &mut self,
        arr: &ast::Array,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        let mut array: Vec<Variable> = Vec::new();
        for item in &arr.items {
            array.push(match try!(self.expression(item, Side::Right, module)) {
                (Some(x), Flow::Continue) => x,
                (x, Flow::Return) => return Ok((x, Flow::Return)),
                _ => return Err(module.error(item.source_range(),
                    &format!("{}\nExpected something",
                        self.stack_trace()), self))
            });
        }
        Ok((Some(Variable::Array(Arc::new(array))), Flow::Continue))
    }

    fn array_fill(
        &mut self,
        array_fill: &ast::ArrayFill,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        let fill = match try!(self.expression(&array_fill.fill, Side::Right, module)) {
            (x, Flow::Return) => return Ok((x, Flow::Return)),
            (Some(x), Flow::Continue) => x,
            _ => return Err(module.error(array_fill.fill.source_range(),
                            &format!("{}\nExpected something",
                                self.stack_trace()), self))
        };
        let n = match try!(self.expression(&array_fill.n, Side::Right, module)) {
            (x, Flow::Return) => return Ok((x, Flow::Return)),
            (Some(x), Flow::Continue) => x,
            _ => return Err(module.error(array_fill.n.source_range(),
                            &format!("{}\nExpected something",
                                self.stack_trace()), self))
        };
        let v = match (self.resolve(&fill), self.resolve(&n)) {
            (x, &Variable::F64(n, _)) => {
                Variable::Array(Arc::new(vec![x.clone(); n as usize]))
            }
            _ => return Err(module.error(array_fill.n.source_range(),
                &format!("{}\nExpected number for length in `[value; length]`",
                    self.stack_trace()), self))
        };
        Ok((Some(v), Flow::Continue))
    }

    fn assign(
        &mut self,
        op: ast::AssignOp,
        left: &ast::Expression,
        right: &ast::Expression,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        use ast::AssignOp::*;
        use ast::Expression;

        if op != Assign {
            // Evaluate right side before left because the left leaves
            // an raw pointer on the stack which might point to wrong place
            // if there are side effects of the right side affecting it.
            let b = match try!(self.expression(right, Side::Right, module)) {
                (Some(x), Flow::Continue) => x,
                (x, Flow::Return) => return Ok((x, Flow::Return)),
                _ => return Err(module.error(right.source_range(),
                        &format!("{}\nExpected something from the right side",
                            self.stack_trace()), self))
            };
            let a = match try!(self.expression(left, Side::LeftInsert(false), module)) {
                (Some(x), Flow::Continue) => x,
                (x, Flow::Return) => return Ok((x, Flow::Return)),
                _ => return Err(module.error(left.source_range(),
                        &format!("{}\nExpected something from the left side",
                            self.stack_trace()), self))
            };
            let r = match a {
                Variable::UnsafeRef(r) => {
                    // If reference, use a shallow clone to type check,
                    // without affecting the original object.
                    unsafe {
                        if let Variable::Ref(ind) = *r.0 {
                            *r.0 = self.stack[ind].clone()
                        }
                    }
                    r
                }
                Variable::Ref(ind) => {
                    UnsafeRef(&mut self.stack[ind] as *mut Variable)
                }
                x => panic!("Expected reference, found `{}`", x.typeof_var())
            };

            match *self.resolve(&b) {
                Variable::F64(b, ref sec) => {
                    unsafe {
                        match *r.0 {
                            Variable::F64(ref mut n, ref mut n_sec) => {
                                match op {
                                    Set => *n = b,
                                    Add => *n += b,
                                    Sub => *n -= b,
                                    Mul => *n *= b,
                                    Div => *n /= b,
                                    Rem => *n %= b,
                                    Pow => *n = n.powf(b),
                                    Assign => {}
                                };
                                *n_sec = sec.clone()
                            }
                            Variable::Return => {
                                if let Set = op {
                                    *r.0 = Variable::F64(b, sec.clone())
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nReturn has no value",
                                            self.stack_trace()), self))
                                }
                            }
                            Variable::Link(ref mut n) => {
                                if let Add = op {
                                    try!(n.push(&Variable::f64(b)));
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nCan not use this assignment \
                                        operator with `link` and `number`",
                                            self.stack_trace()), self));
                                }
                            }
                            _ => return Err(module.error(
                                    left.source_range(),
                                    &format!("{}\nExpected assigning to a number",
                                        self.stack_trace()), self))
                        };
                    }
                }
                Variable::Vec4(b) => {
                    unsafe {
                        match *r.0 {
                            Variable::Vec4(ref mut n) => {
                                match op {
                                    Set => *n = b,
                                    Add => *n = [n[0] + b[0], n[1] + b[1],
                                                 n[2] + b[2], n[3] + b[3]],
                                    Sub => *n = [n[0] - b[0], n[1] - b[1],
                                                 n[2] - b[2], n[3] - b[3]],
                                    Mul => *n = [n[0] * b[0], n[1] * b[1],
                                                 n[2] * b[2], n[3] * b[3]],
                                    Div => *n = [n[0] / b[0], n[1] / b[1],
                                                 n[2] / b[2], n[3] / b[3]],
                                    Rem => *n = [n[0] % b[0], n[1] % b[1],
                                                 n[2] % b[2], n[3] % b[3]],
                                    Pow => *n = [n[0].powf(b[0]), n[1].powf(b[1]),
                                                 n[2].powf(b[2]), n[3].powf(b[3])],
                                    Assign => {}
                                }
                            }
                            Variable::Return => {
                                if let Set = op {
                                    *r.0 = Variable::Vec4(b)
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nReturn has no value",
                                            self.stack_trace()), self))
                                }
                            }
                            _ => return Err(module.error(
                                    left.source_range(),
                                    &format!("{}\nExpected assigning to a vec4",
                                        self.stack_trace()), self))
                        };
                    }
                }
                Variable::Mat4(ref b) => {
                    unsafe {
                        match *r.0 {
                            Variable::Mat4(ref mut n) => {
                                match op {
                                    Set => {
                                        **n = **b;
                                    }
                                    Mul => {
                                        use vecmath::col_mat4_mul;

                                        **n = col_mat4_mul(**n, **b);
                                    }
                                    Add => {
                                        use vecmath::mat4_add;

                                        **n = mat4_add(**n, **b);
                                    }
                                    _ => {
                                        return Err(module.error(
                                            left.source_range(),
                                            &format!("{}\nCan not use this assignment \
                                            operator with `mat4`",
                                                self.stack_trace()), self));
                                    }
                                }
                            }
                            Variable::Return => {
                                if let Set = op {
                                    *r.0 = Variable::Mat4(b.clone())
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nReturn has no value",
                                            self.stack_trace()), self))
                                }
                            }
                            _ => return Err(module.error(
                                    left.source_range(),
                                    &format!("{}\nExpected assigning to a mat4",
                                        self.stack_trace()), self))
                        }
                    }
                }
                Variable::Bool(b, ref sec) => {
                    unsafe {
                        match *r.0 {
                            Variable::Bool(ref mut n, ref mut n_sec) => {
                                match op {
                                    Set => *n = b,
                                    _ => unimplemented!()
                                };
                                *n_sec = sec.clone();
                            }
                            Variable::Return => {
                                if let Set = op {
                                    *r.0 = Variable::Bool(b, sec.clone())
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nReturn has no value",
                                            self.stack_trace()), self))
                                }
                            }
                            Variable::Link(ref mut n) => {
                                if let Add = op {
                                    try!(n.push(&Variable::bool(b)));
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nCan not use this assignment \
                                        operator with `link` and `bool`",
                                            self.stack_trace()), self));
                                }
                            }
                            _ => return Err(module.error(
                                    left.source_range(),
                                    &format!("{}\nExpected assigning to a bool",
                                        self.stack_trace()), self))
                        };
                    }
                }
                Variable::Text(ref b) => {
                    unsafe {
                        match *r.0 {
                            Variable::Text(ref mut n) => {
                                match op {
                                    Set => *n = b.clone(),
                                    Add => Arc::make_mut(n).push_str(b),
                                    _ => unimplemented!()
                                }
                            }
                            Variable::Return => {
                                if let Set = op {
                                    *r.0 = Variable::Text(b.clone())
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nReturn has no value",
                                            self.stack_trace()), self))
                                }
                            }
                            Variable::Link(ref mut n) => {
                                if let Add = op {
                                    try!(n.push(&Variable::Text(b.clone())));
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nCan not use this assignment \
                                        operator with `link` and `text`",
                                            self.stack_trace()), self));
                                }
                            }
                            _ => return Err(module.error(
                                left.source_range(),
                                &format!("{}\nExpected assigning to text",
                                    self.stack_trace()), self))
                        }
                    }
                }
                Variable::Object(ref b) => {
                    unsafe {
                        match *r.0 {
                            Variable::Object(_) => {
                                if let Set = op {
                                    *r.0 = Variable::Object(b.clone())
                                } else {
                                    unimplemented!()
                                }
                            }
                            Variable::Return => {
                                if let Set = op {
                                    *r.0 = Variable::Object(b.clone())
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nReturn has no value",
                                            self.stack_trace()), self))
                                }
                            }
                            _ => return Err(module.error(
                                left.source_range(),
                                &format!("{}\nExpected assigning to object",
                                    self.stack_trace()), self))
                        }
                    }
                }
                Variable::Array(ref b) => {
                    unsafe {
                        match *r.0 {
                            Variable::Array(_) => {
                                if let Set = op {
                                    *r.0 = Variable::Array(b.clone())
                                } else {
                                    unimplemented!()
                                }
                            }
                            Variable::Return => {
                                if let Set = op {
                                    *r.0 = Variable::Array(b.clone())
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nReturn has no value",
                                            self.stack_trace()), self))
                                }
                            }
                            _ => return Err(module.error(
                                left.source_range(),
                                &format!("{}\nExpected assigning to array",
                                    self.stack_trace()), self))
                        }
                    }
                }
                Variable::Link(ref b) => {
                    unsafe {
                        match *r.0 {
                            Variable::Link(ref mut n) => {
                                match op {
                                    Set => *n = b.clone(),
                                    Add => **n = n.add(b),
                                    Sub => **n = b.add(n),
                                    _ => unimplemented!()
                                }
                            }
                            Variable::Return => {
                                if let Set = op {
                                    *r.0 = Variable::Link(b.clone())
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nReturn has no value",
                                            self.stack_trace()), self))
                                }
                            }
                            _ => return Err(module.error(
                                left.source_range(),
                                &format!("{}\nExpected assigning to link",
                                    self.stack_trace()), self))
                        }
                    }
                }
                Variable::Option(ref b) => {
                    unsafe {
                        match *r.0 {
                            Variable::Option(_) => {
                                if let Set = op {
                                    *r.0 = Variable::Option(b.clone())
                                } else {
                                    unimplemented!()
                                }
                            }
                            Variable::Return => {
                                if let Set = op {
                                    *r.0 = Variable::Option(b.clone())
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nReturn has no value",
                                            self.stack_trace()), self))
                                }
                            }
                            _ => return Err(module.error(
                                left.source_range(),
                                &format!("{}\nExpected assigning to option",
                                    self.stack_trace()), self))
                        }
                    }
                }
                Variable::Result(ref b) => {
                    unsafe {
                        match *r.0 {
                            Variable::Result(_) => {
                                if let Set = op {
                                    *r.0 = Variable::Result(b.clone())
                                } else {
                                    unimplemented!()
                                }
                            }
                            Variable::Return => {
                                if let Set = op {
                                    *r.0 = Variable::Result(b.clone())
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nReturn has no value",
                                            self.stack_trace()), self))
                                }
                            }
                            _ => return Err(module.error(
                                left.source_range(),
                                &format!("{}\nExpected assigning to result",
                                    self.stack_trace()), self))
                        }
                    }
                }
                Variable::RustObject(ref b) => {
                    unsafe {
                        match *r.0 {
                            Variable::RustObject(_) => {
                                if let Set = op {
                                    *r.0 = Variable::RustObject(b.clone())
                                } else {
                                    unimplemented!()
                                }
                            }
                            Variable::Return => {
                                if let Set = op {
                                    *r.0 = Variable::RustObject(b.clone())
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nReturn has no value",
                                            self.stack_trace()), self))
                                }
                            }
                            _ => return Err(module.error(
                                left.source_range(),
                                &format!(
                                    "{}\nExpected assigning to rust_object",
                                    self.stack_trace()), self))
                        }
                    }
                }
                Variable::Closure(ref b, ref env) => {
                    unsafe {
                        match *r.0 {
                            Variable::Closure(_, _) => {
                                if let Set = op {
                                    *r.0 = Variable::Closure(b.clone(), env.clone())
                                } else {
                                    unimplemented!()
                                }
                            }
                            Variable::Return => {
                                if let Set = op {
                                    *r.0 = Variable::Closure(b.clone(), env.clone())
                                } else {
                                    return Err(module.error(
                                        left.source_range(),
                                        &format!("{}\nReturn has no value",
                                            self.stack_trace()), self))
                                }
                            }
                            _ => return Err(module.error(
                                left.source_range(),
                                &format!(
                                    "{}\nExpected assigning to closure",
                                    self.stack_trace()), self))
                        }
                    }
                }
                ref x => {
                    return Err(module.error(
                        left.source_range(),
                        &format!("{}\nCan not use this assignment operator with `{}`",
                            self.stack_trace(), x.typeof_var()), self));
                }
            };
            Ok((None, Flow::Continue))
        } else {
            return match *left {
                Expression::Item(ref item) => {
                    let x = match try!(self.expression(right, Side::Right, module)) {
                        (x, Flow::Return) => return Ok((x, Flow::Return)),
                        (Some(x), Flow::Continue) => x,
                        _ => return Err(module.error(right.source_range(),
                                    &format!("{}\nExpected something from the right side",
                                        self.stack_trace()), self))
                    };
                    let v = match x {
                        // Use a shallow clone of a reference.
                        Variable::Ref(ind) => self.stack[ind].clone(),
                        x => x
                    };
                    if item.ids.len() != 0 {
                        let x = match try!(self.expression(left, Side::LeftInsert(true),
                                                   module)) {
                            (Some(x), Flow::Continue) => x,
                            (x, Flow::Return) => return Ok((x, Flow::Return)),
                            _ => return Err(module.error(left.source_range(),
                                    &format!("{}\nExpected something from the left side",
                                        self.stack_trace()), self))
                        };
                        match x {
                            Variable::UnsafeRef(r) => {
                                unsafe { *r.0 = v }
                            }
                            _ => panic!("Expected unsafe reference")
                        }
                    } else {
                        self.local_stack.push((item.name.clone(), self.stack.len()));
                        if item.current {
                            self.current_stack.push((item.name.clone(), self.stack.len()));
                        }
                        self.stack.push(v);
                    }
                    Ok((None, Flow::Continue))
                }
                _ => return Err(module.error(left.source_range(),
                                &format!("{}\nExpected item",
                                    self.stack_trace()), self))
            }
        }
    }
    // `insert` is true for `:=` and false for `=`.
    // This works only on objects, but does not have to check since it is
    // ignored for arrays.
    fn item(
        &mut self,
        item: &ast::Item,
        side: Side,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        use Error;

        #[inline(always)]
        fn try(
            stack: &mut Vec<Variable>,
            call_stack: &Vec<Call>,
            v: Result<Box<Variable>, Box<Error>>,
            source_range: Range,
            module: &Module
        ) -> Result<(Option<Variable>, Flow), String> {
            match v {
                Ok(ok) => Ok((Some(*ok), Flow::Continue)),
                Err(mut err) => {
                    let call = call_stack.last().unwrap();
                    if call.stack_len == 0 {
                        return Err(module.error_fnindex(source_range,
                            &format!("{}\nRequires `->` on function `{}`",
                            stack_trace(call_stack),
                            &call.fn_name), call.index));
                    }
                    if let Variable::Return = stack[call.stack_len - 1] {}
                    else {
                        return Err(module.error_fnindex(source_range,
                            &format!("{}\nRequires `->` on function `{}`",
                            stack_trace(call_stack),
                            &call.fn_name),
                            call.index));
                    }
                    let file = match call.file {
                        None => "".into(),
                        Some(ref f) => format!(" ({})", f)
                    };
                    err.trace.push(module.error_fnindex(
                        source_range,
                        &format!("In function `{}`{}", call.fn_name, file),
                        call.index));
                    Ok((Some(Variable::Result(Err(err))), Flow::Return))
                }
            }
        }

        use ast::Id;

        let locals = self.local_stack.len() - self.call_stack.last().unwrap().local_len;
        let stack_id = {
            if cfg!(not(feature = "debug_resolve")) {
                self.stack.len() - item.static_stack_id.get().unwrap()
            } else {
                match item.stack_id.get() {
                    Some(val) => self.stack.len() - val,
                    None => {
                        let name: &str = &**item.name;
                        let mut found = false;
                        for &(ref n, id) in self.local_stack.iter().rev().take(locals) {
                            if &**n == name {
                                let new_val = Some(self.stack.len() - id);
                                item.stack_id.set(new_val);

                                let static_stack_id = item.static_stack_id.get();
                                if new_val != static_stack_id {
                                    return Err(module.error(item.source_range,
                                        &format!(
                                            "DEBUG: resolved not same for {} `{:?}` vs static `{:?}`",
                                            name,
                                            new_val,
                                            static_stack_id
                                        ), self));
                                }

                                found = true;
                                break;
                            }
                        }
                        if found {
                            self.stack.len() - item.stack_id.get().unwrap()
                        } else if name == "return" {
                            return Err(module.error(item.source_range, &format!(
                                "{}\nRequires `->` on function `{}`",
                                self.stack_trace(),
                                &self.call_stack.last().unwrap().fn_name), self));
                        } else {
                            return Err(module.error(item.source_range, &format!(
                                "{}\nCould not find local or current variable `{}`",
                                    self.stack_trace(), name), self));
                        }
                    }
                }
            }
        };

        if cfg!(feature = "debug_resolve") {
            for &(ref n, id) in self.local_stack.iter().rev().take(locals) {
                if &**n == &**item.name {
                    if stack_id != id {
                        return Err(module.error(item.source_range,
                            &format!("DEBUG: Not same for {} stack_id `{:?}` vs id `{:?}`",
                                item.name,
                                stack_id,
                                id), self));
                    }
                    break;
                }
            }
        }

        let stack_id = if let &Variable::Ref(ref_id) = &self.stack[stack_id] {
                ref_id
            } else {
                stack_id
            };
        if item.ids.len() == 0 {
            if item.try {
                // Check for `err(_)` or unwrap when `?` follows item.
                let v = match Runtime::try_msg(&self.stack[stack_id]) {
                    Some(v) => v,
                    None => {
                        return Err(module.error(item.source_range,
                            &format!("{}\nExpected `ok(_)`, `err(_)`, `bool`, `f64`",
                                self.stack_trace()), self));
                    }
                };
                return try(&mut self.stack, &self.call_stack, v,
                           item.source_range, module);
            } else {
                return Ok((Some(Variable::Ref(stack_id)), Flow::Continue));
            }
        }

        // Pre-evaluate expressions for identity.
        let start_stack_len = self.stack.len();
        for id in &item.ids {
            if let &Id::Expression(ref expr) = id {
                match try!(self.expression(expr, Side::Right, module)) {
                    (x, Flow::Return) => return Ok((x, Flow::Return)),
                    (Some(x), Flow::Continue) => self.stack.push(x),
                    _ => return Err(module.error(expr.source_range(),
                        &format!("{}\nExpected something for index",
                            self.stack_trace()), self))
                };
            }
        }
        let &mut Runtime {
            ref mut stack,
            ref mut call_stack,
            ..
        } = self;
        let mut expr_j = 0;
        let insert = match side {
            Side::Right => false,
            Side::LeftInsert(insert) => insert,
        };

        let v = {
            let item_len = item.ids.len();
            // Get the first variable (a.x).y
            let mut var: *mut Variable = try!(item_lookup(
                module,
                &mut stack[stack_id],
                stack,
                call_stack,
                &item.ids[0],
                start_stack_len,
                &mut expr_j,
                insert,
                item_len == 1
            ));
            let mut try_id_ind = 0;
            if item.try_ids.len() > 0 && item.try_ids[try_id_ind] == 0 {
                // Check for error on `?` for first id.
                let v = unsafe {match Runtime::try_msg(&*var) {
                    Some(v) => v,
                    None => {
                        return Err(module.error_fnindex(item.ids[0].source_range(),
                            &format!("{}\nExpected `ok(_)` or `err(_)`",
                                stack_trace(call_stack)),
                                call_stack.last().unwrap().index));
                    }
                }};
                match v {
                    Ok(ref ok) => unsafe {
                        *var = (**ok).clone();
                        try_id_ind += 1;
                    },
                    Err(ref err) => {
                        let call = call_stack.last().unwrap();
                        if call.stack_len == 0 {
                            return Err(module.error_fnindex(
                                item.ids[0].source_range(),
                                &format!("{}\nRequires `->` on function `{}`",
                                stack_trace(call_stack),
                                &call.fn_name),
                                call.index));
                        }
                        if let Variable::Return = stack[call.stack_len - 1] {}
                        else {
                            return Err(module.error_fnindex(
                                item.ids[0].source_range(),
                                &format!("{}\nRequires `->` on function `{}`",
                                stack_trace(call_stack),
                                &call.fn_name),
                                call.index));
                        }
                        let mut err = err.clone();
                        let file = match call.file.as_ref() {
                            None => "".into(),
                            Some(f) => format!(" ({})", f)
                        };
                        err.trace.push(module.error_fnindex(
                            item.ids[0].source_range(),
                            &format!("In function `{}`{}",
                                &call.fn_name, file),
                                call.index));
                        return Ok((Some(Variable::Result(Err(err))), Flow::Return));
                    }
                }
            }
            // Get the rest of the variables.
            for (i, prop) in item.ids[1..].iter().enumerate() {
                var = try!(item_lookup(
                    module,
                    unsafe { &mut *var },
                    stack,
                    call_stack,
                    prop,
                    start_stack_len,
                    &mut expr_j,
                    insert,
                    // `i` skips first index.
                    i + 2 == item_len
                ));

                if item.try_ids.len() > try_id_ind &&
                   item.try_ids[try_id_ind] == i + 1 {
                    // Check for error on `?` for rest of ids.
                    let v = unsafe {match Runtime::try_msg(&*var) {
                        Some(v) => v,
                        None => {
                            return Err(module.error_fnindex(prop.source_range(),
                                &format!("{}\nExpected `ok(_)`, `err(_)`, `bool`, `f64`",
                                    stack_trace(call_stack)),
                                    call_stack.last().unwrap().index));
                        }
                    }};
                    match v {
                        Ok(ref ok) => unsafe {
                            *var = (**ok).clone();
                            try_id_ind += 1;
                        },
                        Err(ref err) => {
                            let call = call_stack.last().unwrap();
                            if call.stack_len == 0 {
                                return Err(module.error_fnindex(
                                    prop.source_range(),
                                    &format!("{}\nRequires `->` on function `{}`",
                                        stack_trace(call_stack),
                                        &call.fn_name),
                                        call.index));
                            }
                            if let Variable::Return = stack[call.stack_len - 1] {}
                            else {
                                return Err(module.error_fnindex(
                                    prop.source_range(),
                                    &format!("{}\nRequires `->` on function `{}`",
                                        stack_trace(call_stack),
                                        &call.fn_name),
                                        call.index));
                            }
                            let mut err = err.clone();
                            let file = match call.file.as_ref() {
                                None => "".into(),
                                Some(f) => format!(" ({})", f)
                            };
                            err.trace.push(module.error_fnindex(
                                prop.source_range(),
                                &format!("In function `{}`{}",
                                    &call.fn_name, file),
                                    call.index));
                            return Ok((Some(Variable::Result(Err(err))), Flow::Return));
                        }
                    }
                }
            }

            match side {
                Side::Right => unsafe {&*var}.clone(),
                Side::LeftInsert(_) => Variable::UnsafeRef(UnsafeRef(var))
            }
        };
        stack.truncate(start_stack_len);
        return Ok((Some(v), Flow::Continue));
    }

    fn compare(
        &mut self,
        compare: &ast::Compare,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        fn sub_compare(
            rt: &Runtime,
            compare: &ast::Compare,
            module: &Module,
            a: &Variable,
            b: &Variable
        ) -> Result<Variable, String> {
            use ast::CompareOp::*;

            match (rt.resolve(&b), rt.resolve(&a)) {
                (&Variable::F64(b, _), &Variable::F64(a, ref sec)) => {
                    Ok(Variable::Bool(match compare.op {
                        Less => a < b,
                        LessOrEqual => a <= b,
                        Greater => a > b,
                        GreaterOrEqual => a >= b,
                        Equal => a == b,
                        NotEqual => a != b
                    }, sec.clone()))
                }
                (&Variable::Text(ref b), &Variable::Text(ref a)) => {
                    Ok(Variable::bool(match compare.op {
                        Less => a < b,
                        LessOrEqual => a <= b,
                        Greater => a > b,
                        GreaterOrEqual => a >= b,
                        Equal => a == b,
                        NotEqual => a != b
                    }))
                }
                (&Variable::Bool(b, _), &Variable::Bool(a, ref sec)) => {
                    Ok(Variable::Bool(match compare.op {
                        Equal => a == b,
                        NotEqual => a != b,
                        x => return Err(module.error(compare.source_range,
                            &format!("{}\n`{}` can not be used with bools",
                                rt.stack_trace(),
                                x.symbol()), rt))
                    }, sec.clone()))
                }
                (&Variable::Vec4(ref b), &Variable::Vec4(ref a)) => {
                    Ok(Variable::bool(match compare.op {
                        Equal => a == b,
                        NotEqual => a != b,
                        x => return Err(module.error(compare.source_range,
                            &format!("{}\n`{}` can not be used with vec4s",
                                rt.stack_trace(),
                                x.symbol()), rt))
                    }))
                }
                (&Variable::Object(ref b), &Variable::Object(ref a)) => {
                    Ok(Variable::bool(match compare.op {
                        Equal => {
                            a.len() == b.len() &&
                            a.iter().all(|a| {
                                if let Some(b_val) = b.get(a.0) {
                                    if let Ok(Variable::Bool(true, _)) =
                                        sub_compare(rt, compare, module, &a.1, b_val) {true}
                                    else {false}
                                } else {false}
                            })
                        }
                        NotEqual => {
                            a.len() != b.len() ||
                            a.iter().any(|a| {
                                if let Some(b_val) = b.get(a.0) {
                                    if let Ok(Variable::Bool(false, _)) =
                                        sub_compare(rt, compare, module, &a.1, b_val) {false}
                                    else {true}
                                } else {true}
                            })
                        }
                        x => return Err(module.error(compare.source_range,
                            &format!("{}\n`{}` can not be used with objects",
                                rt.stack_trace(),
                                x.symbol()), rt))
                    }))
                }
                (&Variable::Array(ref b), &Variable::Array(ref a)) => {
                    Ok(Variable::bool(match compare.op {
                        Equal => {
                            a.len() == b.len() &&
                            a.iter().zip(b.iter()).all(|(a, b)| {
                                if let Ok(Variable::Bool(true, _)) =
                                    sub_compare(rt, compare, module, a, b) {true} else {false}
                            })
                        }
                        NotEqual => {
                            a.len() != b.len() ||
                            a.iter().zip(b.iter()).any(|(a, b)| {
                                if let Ok(Variable::Bool(false, _)) =
                                    sub_compare(rt, compare, module, a, b) {false} else {true}
                            })
                        }
                        x => return Err(module.error(compare.source_range,
                            &format!("{}\n`{}` can not be used with arrays",
                                rt.stack_trace(),
                                x.symbol()), rt))
                    }))
                }
                (&Variable::Option(None), &Variable::Option(None)) => {
                    Ok(Variable::bool(match compare.op {
                        Equal => true,
                        NotEqual => false,
                        x => return Err(module.error(compare.source_range,
                            &format!("{}\n`{}` can not be used with options",
                                rt.stack_trace(),
                                x.symbol()), rt))
                    }))
                }
                (&Variable::Option(None), &Variable::Option(_)) => {
                    Ok(Variable::bool(match compare.op {
                        Equal => false,
                        NotEqual => true,
                        x => return Err(module.error(compare.source_range,
                            &format!("{}\n`{}` can not be used with options",
                                rt.stack_trace(),
                                x.symbol()), rt))
                    }))
                }
                (&Variable::Option(_), &Variable::Option(None)) => {
                    Ok(Variable::bool(match compare.op {
                        Equal => false,
                        NotEqual => true,
                        x => return Err(module.error(compare.source_range,
                            &format!("{}\n`{}` can not be used with options",
                                rt.stack_trace(),
                                x.symbol()), rt))
                    }))
                }
                (&Variable::Option(Some(ref b)),
                 &Variable::Option(Some(ref a))) => {
                    sub_compare(rt, compare, module, a, b)
                }
                (b, a) => return Err(module.error(compare.source_range,
                    &format!(
                    "{}\n`{}` can not be used with `{}` and `{}`",
                    rt.stack_trace(),
                    compare.op.symbol(),
                    a.typeof_var(),
                    b.typeof_var()), rt))
            }
        }

        let left = match try!(self.expression(&compare.left, Side::Right, module)) {
            (Some(x), Flow::Continue) => x,
            (x, Flow::Return) => { return Ok((x, Flow::Return)); }
            _ => return Err(module.error(compare.left.source_range(),
                &format!("{}\nExpected something from the left argument",
                    self.stack_trace()), self))
        };
        let right = match try!(self.expression(&compare.right, Side::Right, module)) {
            (Some(x), Flow::Continue) => x,
            (x, Flow::Return) => return Ok((x, Flow::Return)),
            _ => return Err(module.error(compare.right.source_range(),
                &format!("{}\nExpected something from the right argument",
                    self.stack_trace()), self))
        };
        Ok((Some(try!(sub_compare(self, compare, module, &left, &right))), Flow::Continue))
    }
    fn if_expr(
        &mut self,
        if_expr: &ast::If,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        let cond = match try!(self.expression(&if_expr.cond, Side::Right, module)) {
            (Some(x), Flow::Continue) => x,
            (x, Flow::Return) => { return Ok((x, Flow::Return)); }
            _ => return Err(module.error(if_expr.cond.source_range(),
                &format!("{}\nExpected bool from if condition",
                    self.stack_trace()), self))
        };
        let val = match self.resolve(&cond) {
            &Variable::Bool(val, _) => val,
            _ => return Err(module.error(if_expr.cond.source_range(),
                &format!("{}\nExpected bool from if condition",
                    self.stack_trace()), self))
        };
        if val {
            return self.block(&if_expr.true_block, module);
        }
        for (cond, body) in if_expr.else_if_conds.iter()
            .zip(if_expr.else_if_blocks.iter()) {
            let else_if_cond = match try!(self.expression(cond, Side::Right, module)) {
                (Some(x), Flow::Continue) => x,
                (x, Flow::Return) => {
                    return Ok((x, Flow::Return));
                }
                _ => return Err(module.error(cond.source_range(),
                    &format!("{}\nExpected bool from else if condition",
                        self.stack_trace()), self))
            };
            match self.resolve(&else_if_cond) {
                &Variable::Bool(false, _) => {}
                &Variable::Bool(true, _) => {
                    return self.block(body, module);
                }
                _ => return Err(module.error(cond.source_range(),
                    &format!("{}\nExpected bool from else if condition",
                        self.stack_trace()), self))
            }
        }
        if let Some(ref block) = if_expr.else_block {
            self.block(block, module)
        } else {
            Ok((None, Flow::Continue))
        }
    }
    fn for_expr(
        &mut self,
        for_expr: &ast::For,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        let prev_st = self.stack.len();
        let prev_lc = self.local_stack.len();
        match try!(self.expression(&for_expr.init, Side::Right, module)) {
        (None, Flow::Continue) => {}
            (x, Flow::Return) => { return Ok((x, Flow::Return)); }
            _ => return Err(module.error(for_expr.init.source_range(),
                &format!("{}\nExpected nothing from for init",
                    self.stack_trace()), self))
        };
        let st = self.stack.len();
        let lc = self.local_stack.len();
        let mut flow = Flow::Continue;
        loop {
            let val = match try!(self.expression(&for_expr.cond, Side::Right, module)) {
                (Some(x), Flow::Continue) => x,
                (x, Flow::Return) => return Ok((x, Flow::Return)),
                _ => return Err(module.error(for_expr.cond.source_range(),
                    &format!("{}\nExpected bool from for condition",
                        self.stack_trace()), self))
            };
            let val = match val {
                Variable::Bool(val, _) => val,
                _ => return Err(module.error(
                    for_expr.cond.source_range(),
                    &format!("{}\nExpected bool", self.stack_trace()), self))
            };
            if !val { break }
            match try!(self.block(&for_expr.block, module)) {
                (x, Flow::Return) => return Ok((x, Flow::Return)),
                (_, Flow::Continue) => {}
                (_, Flow::Break(x)) => {
                    match x {
                        Some(label) => {
                            let same =
                            if let Some(ref for_label) = for_expr.label {
                                &label == for_label
                            } else { false };
                            if !same {
                                flow = Flow::Break(Some(label))
                            }
                        }
                        None => {}
                    }
                    break;
                }
                (_, Flow::ContinueLoop(x)) => {
                    match x {
                        Some(label) => {
                            let same =
                            if let Some(ref for_label) = for_expr.label {
                                &label == for_label
                            } else { false };
                            if !same {
                                flow = Flow::ContinueLoop(Some(label));
                                break;
                            }
                        }
                        None => {}
                    }
                    match try!(self.expression(&for_expr.step, Side::Right, module)) {
                        (None, Flow::Continue) => {}
                        (x, Flow::Return) => return Ok((x, Flow::Return)),
                        _ => return Err(module.error(
                            for_expr.step.source_range(),
                            &format!("{}\nExpected nothing from for step",
                                self.stack_trace()), self))
                    };
                    continue;
                }
            }
            match try!(self.expression(&for_expr.step, Side::Right, module)) {
                (None, Flow::Continue) => {}
                (x, Flow::Return) => return Ok((x, Flow::Return)),
                _ => return Err(module.error(
                    for_expr.step.source_range(),
                    &format!("{}\nExpected nothing from for step",
                        self.stack_trace()), self))
            };
            self.stack.truncate(st);
            self.local_stack.truncate(lc);
        };
        self.stack.truncate(prev_st);
        self.local_stack.truncate(prev_lc);
        Ok((None, flow))
    }
    fn vec4(
        &mut self,
        vec4: &ast::Vec4,
        side: Side,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        let st = self.stack.len();
        for expr in &vec4.args {
            match try!(self.expression(expr, side, module)) {
                (None, Flow::Continue) => {}
                (Some(x), Flow::Continue) => self.stack.push(x),
                (x, Flow::Return) => return Ok((x, Flow::Return)),
                _ => return Err(module.error(expr.source_range(),
                    &format!("{}\nExpected something from vec4 argument",
                        self.stack_trace()), self))
            };
            // Skip the rest if swizzling pushes arguments.
            if self.stack.len() - st > 3 { break; }
        }
        let w = self.stack.pop().expect(TINVOTS);
        let w = match self.resolve(&w) {
            &Variable::F64(val, _) => val,
            x => return Err(module.error(vec4.args[3].source_range(),
                &self.expected(x, "number"), self))
        };
        let z = self.stack.pop().expect(TINVOTS);
        let z = match self.resolve(&z) {
            &Variable::F64(val, _) => val,
            x => return Err(module.error(vec4.args[2].source_range(),
                &self.expected(x, "number"), self))
        };
        let y = self.stack.pop().expect(TINVOTS);
        let y = match self.resolve(&y) {
            &Variable::F64(val, _) => val,
            x => return Err(module.error(vec4.args[1].source_range(),
                &self.expected(x, "number"), self))
        };
        let x = self.stack.pop().expect(TINVOTS);
        let x = match self.resolve(&x) {
            &Variable::F64(val, _) => val,
            x => return Err(module.error(vec4.args[0].source_range(),
                &self.expected(x, "number"), self))
        };
        Ok((Some(Variable::Vec4([x as f32, y as f32, z as f32, w as f32])), Flow::Continue))
    }
    fn mat4(
        &mut self,
        mat4: &ast::Mat4,
        side: Side,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        for expr in &mat4.args {
            match try!(self.expression(expr, side, module)) {
                (None, Flow::Continue) => {}
                (Some(x), Flow::Continue) => self.stack.push(x),
                (x, Flow::Return) => return Ok((x, Flow::Return)),
                _ => return Err(module.error(expr.source_range(),
                    &format!("{}\nExpected something from mat4 argument",
                        self.stack_trace()), self))
            };
        }
        let w = self.stack.pop().expect(TINVOTS);
        let w = match self.resolve(&w) {
            &Variable::Vec4(val) => val,
            x => return Err(module.error(mat4.args[3].source_range(),
                &self.expected(x, "vec4"), self))
        };
        let z = self.stack.pop().expect(TINVOTS);
        let z = match self.resolve(&z) {
            &Variable::Vec4(val) => val,
            x => return Err(module.error(mat4.args[2].source_range(),
                &self.expected(x, "vec4"), self))
        };
        let y = self.stack.pop().expect(TINVOTS);
        let y = match self.resolve(&y) {
            &Variable::Vec4(val) => val,
            x => return Err(module.error(mat4.args[1].source_range(),
                &self.expected(x, "vec4"), self))
        };
        let x = self.stack.pop().expect(TINVOTS);
        let x = match self.resolve(&x) {
            &Variable::Vec4(val) => val,
            x => return Err(module.error(mat4.args[0].source_range(),
                &self.expected(x, "vec4"), self))
        };
        Ok((Some(Variable::Mat4(Box::new([
            [x[0], y[0], z[0], w[0]],
            [x[1], y[1], z[1], w[1]],
            [x[2], y[2], z[2], w[2]],
            [x[3], y[3], z[3], w[3]],
        ]))), Flow::Continue))
    }
    fn norm(
        &mut self,
        norm: &ast::Norm,
        side: Side,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        let val = match try!(self.expression(&norm.expr, side, module)) {
            (Some(x), Flow::Continue) => x,
            (x, Flow::Return) => return Ok((x, Flow::Return)),
            _ => return Err(module.error(norm.source_range,
                &format!("{}\nExpected something from unary argument",
                    self.stack_trace()), self))
        };
        let v = match self.resolve(&val) {
            &Variable::Vec4(b) => {
                Variable::f64((b[0] * b[0] + b[1] * b[1] + b[2] * b[2]).sqrt() as f64)
            }
            x => return Err(module.error(norm.source_range,
                &self.expected(x, "vec4"), self))
        };
        Ok((Some(v), Flow::Continue))
    }
    fn unop(
        &mut self,
        unop: &ast::UnOpExpression,
        side: Side,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        let val = match try!(self.expression(&unop.expr, side, module)) {
            (Some(x), Flow::Continue) => x,
            (x, Flow::Return) => return Ok((x, Flow::Return)),
            _ => return Err(module.error(unop.source_range,
                &format!("{}\nExpected something from unary argument",
                    self.stack_trace()), self))
        };
        let v = match self.resolve(&val) {
            &Variable::Bool(b, ref sec) => {
                Variable::Bool(match unop.op {
                    ast::UnOp::Not => !b,
                    _ => return Err(module.error(unop.source_range,
                                    &format!("{}\nUnknown boolean unary operator",
                                             self.stack_trace()), self))
                }, sec.clone())
            }
            &Variable::F64(v, ref sec) => {
                Variable::F64(match unop.op {
                    ast::UnOp::Neg => -v,
                    _ => return Err(module.error(unop.source_range,
                                    &format!("{}\nUnknown number unary operator",
                                             self.stack_trace()), self))
                }, sec.clone())
            }
            _ => return Err(module.error(unop.source_range,
                &format!("{}\nInvalid type, expected bool", self.stack_trace()), self))
        };
        Ok((Some(v), Flow::Continue))
    }
    fn binop(
        &mut self,
        binop: &ast::BinOpExpression,
        side: Side,
        module: &Arc<Module>
    ) -> Result<(Option<Variable>, Flow), String> {
        use ast::BinOp::*;

        let left = match try!(self.expression(&binop.left, side, module)) {
            (Some(x), Flow::Continue) => x,
            (x, Flow::Return) => return Ok((x, Flow::Return)),
            _ => return Err(module.error(binop.source_range,
                &format!("{}\nExpected something from left argument",
                    self.stack_trace()), self))
        };

        // Check lazy boolean expressions.
        match binop.op {
            OrElse => {
                if let &Variable::Bool(true, ref sec) = self.resolve(&left) {
                    return Ok((Some(Variable::Bool(true, sec.clone())), Flow::Continue));
                }
            }
            AndAlso => {
                if let &Variable::Bool(false, ref sec) = self.resolve(&left) {
                    return Ok((Some(Variable::Bool(false, sec.clone())), Flow::Continue));
                }
            }
            _ => {}
        }

        let right = match try!(self.expression(&binop.right, side, module)) {
            (Some(x), Flow::Continue) => x,
            (x, Flow::Return) => return Ok((x, Flow::Return)),
            _ => return Err(module.error(binop.source_range,
                &format!("{}\nExpected something from right argument",
                    self.stack_trace()), self))
        };
        let v = match (self.resolve(&left), self.resolve(&right)) {
            (&Variable::F64(a, ref sec), &Variable::F64(b, _)) => {
                Variable::F64(match binop.op {
                    Add => a + b,
                    Sub => a - b,
                    Mul => a * b,
                    Div => a / b,
                    Rem => a % b,
                    Pow => a.powf(b),
                    _ => return Err(module.error(binop.source_range,
                        &format!("{}\nUnknown number operator `{:?}`",
                            self.stack_trace(),
                            binop.op.symbol()), self))
                }, sec.clone())
            }
            (&Variable::Vec4(a), &Variable::Vec4(b)) => {
                match binop.op {
                    Add => Variable::Vec4([a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]]),
                    Sub => Variable::Vec4([a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]),
                    Mul => Variable::Vec4([a[0] * b[0], a[1] * b[1], a[2] * b[2], a[3] * b[3]]),
                    Dot => Variable::f64((a[0] * b[0] + a[1] * b[1] +
                                          a[2] * b[2] + a[3] * b[3]) as f64),
                    Cross => Variable::Vec4([a[1] * b[2] - a[2] * b[1],
                                             a[2] * b[0] - a[0] * b[2],
                                             a[0] * b[1] - a[1] * b[0], 0.0]),
                    Div => Variable::Vec4([a[0] / b[0], a[1] / b[1], a[2] / b[2], a[3] / b[3]]),
                    Rem => Variable::Vec4([a[0] % b[0], a[1] % b[1], a[2] % b[2], a[3] % b[3]]),
                    Pow => Variable::Vec4([a[0].powf(b[0]), a[1].powf(b[1]),
                                           a[2].powf(b[2]), a[3].powf(b[3])]),
                    AndAlso | OrElse => return Err(module.error(binop.source_range,
                        &format!("{}\nUnknown operator `{:?}` for `vec4` and `vec4`",
                            self.stack_trace(),
                            binop.op.symbol_bool()), self)),
                }
            }
            (&Variable::Vec4(a), &Variable::F64(b, _)) => {
                let b = b as f32;
                match binop.op {
                    Add => Variable::Vec4([a[0] + b, a[1] + b, a[2] + b, a[3] + b]),
                    Sub => Variable::Vec4([a[0] - b, a[1] - b, a[2] - b, a[3] - b]),
                    Mul => Variable::Vec4([a[0] * b, a[1] * b, a[2] * b, a[3] * b]),
                    Dot => Variable::f64((a[0] * b + a[1] * b +
                                          a[2] * b + a[3] * b) as f64),
                    Cross => return Err(module.error(binop.source_range,
                        &format!("{}\nExpected two vec4 for `{:?}`",
                            self.stack_trace(), binop.op.symbol()), self)),
                    Div => Variable::Vec4([a[0] / b, a[1] / b, a[2] / b, a[3] / b]),
                    Rem => Variable::Vec4([a[0] % b, a[1] % b, a[2] % b, a[3] % b]),
                    Pow => Variable::Vec4([a[0].powf(b), a[1].powf(b),
                                           a[2].powf(b), a[3].powf(b)]),
                    AndAlso | OrElse => return Err(module.error(binop.source_range,
                        &format!("{}\nUnknown operator `{:?}` for `vec4` and `f64`",
                            self.stack_trace(),
                            binop.op.symbol_bool()), self)),
                }
            }
            (&Variable::F64(a, _), &Variable::Vec4(b)) => {
                let a = a as f32;
                match binop.op {
                    Add => Variable::Vec4([a + b[0], a + b[1], a + b[2], a + b[3]]),
                    Sub => Variable::Vec4([a - b[0], a - b[1], a - b[2], a - b[3]]),
                    Mul => Variable::Vec4([a * b[0], a * b[1], a * b[2], a * b[3]]),
                    Dot => Variable::f64((a * b[0] + a * b[1] +
                                          a * b[2] + a * b[3]) as f64),
                    Div => Variable::Vec4([a / b[0], a / b[1], a / b[2], a / b[3]]),
                    Rem => Variable::Vec4([a % b[0], a % b[1], a % b[2], a % b[3]]),
                    Pow => Variable::Vec4([a.powf(b[0]), a.powf(b[1]),
                                           a.powf(b[2]), a.powf(b[3])]),
                    Cross => return Err(module.error(binop.source_range,
                        &format!("{}\nExpected two vec4 for `{:?}`",
                            self.stack_trace(), binop.op.symbol()), self)),
                    AndAlso | OrElse => return Err(module.error(binop.source_range,
                        &format!("{}\nUnknown operator `{:?}` for `f64` and `vec4`",
                            self.stack_trace(),
                            binop.op.symbol_bool()), self)),
                }
            }
            (&Variable::Mat4(ref a), &Variable::Mat4(ref b)) => {
                use vecmath::{mat4_add, col_mat4_mul};

                match binop.op {
                    Add => Variable::Mat4(Box::new(mat4_add(**a, **b))),
                    Mul => Variable::Mat4(Box::new(col_mat4_mul(**a, **b))),
                    _ => return Err(module.error(binop.source_range,
                        &format!("{}\nUnknown operator `{:?}` for `mat4` and `mat4`",
                            self.stack_trace(),
                            binop.op.symbol_bool()), self)),
                }
            }
            (&Variable::Mat4(ref a), &Variable::Vec4(b)) => {
                use vecmath::col_mat4_transform;

                match binop.op {
                    Mul => Variable::Vec4(col_mat4_transform(**a, b)),
                    _ => return Err(module.error(binop.source_range,
                        &format!("{}\nUnknown operator `{:?}` for `mat4` and `vec4`",
                            self.stack_trace(),
                            binop.op.symbol_bool()), self)),
                }
            }
            (&Variable::Bool(a, ref sec), &Variable::Bool(b, _)) => {
                Variable::Bool(match binop.op {
                    Add | OrElse => a || b,
                    // Boolean subtraction with lazy precedence.
                    Sub => a && !b,
                    Mul | AndAlso => a && b,
                    Pow => a ^ b,
                    _ => return Err(module.error(binop.source_range,
                        &format!("{}\nUnknown boolean operator `{:?}`",
                            self.stack_trace(),
                            binop.op.symbol_bool()), self))
                }, sec.clone())
            }
            (&Variable::Text(ref a), &Variable::Text(ref b)) => {
                match binop.op {
                    Add => {
                        let mut res = String::with_capacity(a.len() + b.len());
                        res.push_str(a);
                        res.push_str(b);
                        Variable::Text(Arc::new(res))
                    }
                    _ => return Err(module.error(binop.source_range,
                        &format!("{}\nThis operation can not be used with strings",
                            self.stack_trace()), self))
                }
            }
            (&Variable::Text(_), _) =>
                return Err(module.error(binop.source_range,
                &format!("{}\nThe right argument must be a string. \
                Try the `str` function", self.stack_trace()), self)),
            (&Variable::Link(ref a), &Variable::Link(ref b)) => {
                match binop.op {
                    Add => {
                        Variable::Link(Box::new(a.add(b)))
                    }
                    _ => return Err(module.error(binop.source_range,
                        &format!("{}\nThis operation can not be used with links",
                            self.stack_trace()), self))
                }
            }
            _ => return Err(module.error(binop.source_range, &format!(
                "{}\nInvalid type for binary operator `{:?}`, \
                expected numbers, vec4s, bools or strings",
                self.stack_trace(),
                binop.op.symbol()), self))
        };

        Ok((Some(v), Flow::Continue))
    }
    pub(crate) fn stack_trace(&self) -> String {
        stack_trace(&self.call_stack)
    }
}

fn stack_trace(call_stack: &[Call]) -> String {
    let mut s = String::new();
    for call in call_stack.iter() {
        s.push_str(&call.fn_name);
        if let Some(ref file) = call.file {
            s.push_str(" (");
            s.push_str(file);
            s.push(')');
        }
        s.push('\n')
    }
    s
}
