use std::sync::Arc;

use super::{AssignOp, Block, Call, CallClosure, CallInfo, Expression, ForN, Id, Item};
use crate::FnIndex;

pub fn infer(block: &Block, name: &str) -> Option<Expression> {
    let mut decls: Vec<Arc<String>> = vec![];
    let list: Option<Item> = infer_block(block, name, &mut decls);
    let res = list.map(|item| {
        let source_range = item.source_range;
        Expression::Call(Box::new(Call {
            f_index: FnIndex::None,
            args: vec![Expression::Item(Box::new(item))],
            custom_source: None,
            info: Box::new(CallInfo {
                alias: None,
                name: Arc::new("len".into()),
                source_range,
            }),
        }))
    });
    res
}

fn infer_expr(expr: &Expression, name: &str, decls: &mut Vec<Arc<String>>) -> Option<Item> {
    use super::Expression::*;

    match *expr {
        Link(ref link) => {
            for expr in &link.items {
                let res = infer_expr(expr, name, decls);
                if res.is_some() {
                    return res;
                }
            }
        }
        Item(ref item) => {
            let res = infer_item(item, name, decls);
            if res.is_some() {
                return res;
            }
        }
        Assign(ref assign_expr) => {
            let left = infer_expr(&assign_expr.left, name, decls);
            if left.is_some() {
                return left;
            }
            let right = infer_expr(&assign_expr.right, name, decls);
            if right.is_some() {
                return right;
            }
        }
        Object(ref obj) => {
            for &(_, ref v) in &obj.key_values {
                let res = infer_expr(v, name, decls);
                if res.is_some() {
                    return res;
                }
            }
        }
        Array(ref arr) => {
            for expr in &arr.items {
                let res = infer_expr(expr, name, decls);
                if res.is_some() {
                    return res;
                }
            }
        }
        ArrayFill(ref arr_fill) => {
            let fill = infer_expr(&arr_fill.fill, name, decls);
            if fill.is_some() {
                return fill;
            }
            let n = infer_expr(&arr_fill.n, name, decls);
            if n.is_some() {
                return n;
            }
        }
        Return(ref ret_expr) => {
            let res = infer_expr(ret_expr, name, decls);
            if res.is_some() {
                return res;
            }
        }
        ReturnVoid(_) => {}
        Break(_) => {}
        Continue(_) => {}
        Block(ref block) => {
            let res = infer_block(block, name, decls);
            if res.is_some() {
                return res;
            }
        }
        #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
        Go(ref go) => {
            let res = infer_call(&go.call, name, decls);
            if res.is_some() {
                return res;
            }
        }
        #[cfg(not(all(not(target_family = "wasm"), feature = "threading")))]
        Go(ref go) => match **go {},
        Call(ref call) => {
            let res = infer_call(call, name, decls);
            if res.is_some() {
                return res;
            }
        }
        CallVoid(_) => unimplemented!("`CallVoid` is transformed from `Call` later"),
        CallReturn(_) => unimplemented!("`CallReturn` is transformed from `Call` later"),
        CallLazy(_) => unimplemented!("`CallLazy` is transformed from `Call` later"),
        CallLoaded(_) => unimplemented!("`CallLoaded` is transformed from `Call` later"),
        CallBinOp(_) => unimplemented!("`CallBinOp` is transformed from `Call` later"),
        CallUnOp(_) => unimplemented!("`CallUnOp` is transformed from `Call` later"),
        Vec4(ref vec4_expr) => {
            for expr in &vec4_expr.args {
                let res = infer_expr(expr, name, decls);
                if res.is_some() {
                    return res;
                }
            }
        }
        Mat4(ref mat4_expr) => {
            for expr in &mat4_expr.args {
                let res = infer_expr(expr, name, decls);
                if res.is_some() {
                    return res;
                }
            }
        }
        For(ref for_expr) => {
            // TODO: Declaring counter with same name probably leads to a bug.
            let res = infer_expr(&for_expr.init, name, decls);
            if res.is_some() {
                return res;
            }
            let res = infer_expr(&for_expr.cond, name, decls);
            if res.is_some() {
                return res;
            }
            let res = infer_expr(&for_expr.step, name, decls);
            if res.is_some() {
                return res;
            }
            let res = infer_block(&for_expr.block, name, decls);
            if res.is_some() {
                return res;
            }
        }
        ForN(ref for_n_expr) => return infer_for_n(for_n_expr, name, decls),
        #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
        ForIn(ref for_in_expr) => {
            let res = infer_expr(&for_in_expr.iter, name, decls);
            if res.is_some() {
                return res;
            }
        }
        #[cfg(not(all(not(target_family = "wasm"), feature = "threading")))]
        ForIn(ref for_in_expr) |
        SumIn(ref for_in_expr) |
        ProdIn(ref for_in_expr) |
        MinIn(ref for_in_expr) |
        MaxIn(ref for_in_expr) |
        SiftIn(ref for_in_expr) |
        AnyIn(ref for_in_expr) |
        AllIn(ref for_in_expr) |
        LinkIn(ref for_in_expr) => match **for_in_expr {},
        Sum(ref for_n_expr) => return infer_for_n(for_n_expr, name, decls),
        #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
        SumIn(ref for_in_expr) => {
            let res = infer_expr(&for_in_expr.iter, name, decls);
            if res.is_some() {
                return res;
            }
        }
        #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
        ProdIn(ref for_in_expr) => {
            let res = infer_expr(&for_in_expr.iter, name, decls);
            if res.is_some() {
                return res;
            }
        }
        #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
        MinIn(ref for_in_expr) => {
            let res = infer_expr(&for_in_expr.iter, name, decls);
            if res.is_some() {
                return res;
            }
        }
        #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
        MaxIn(ref for_in_expr) => {
            let res = infer_expr(&for_in_expr.iter, name, decls);
            if res.is_some() {
                return res;
            }
        }
        #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
        AnyIn(ref for_in_expr) => {
            let res = infer_expr(&for_in_expr.iter, name, decls);
            if res.is_some() {
                return res;
            }
        }
        #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
        AllIn(ref for_in_expr) => {
            let res = infer_expr(&for_in_expr.iter, name, decls);
            if res.is_some() {
                return res;
            }
        }
        #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
        SiftIn(ref for_in_expr) => {
            let res = infer_expr(&for_in_expr.iter, name, decls);
            if res.is_some() {
                return res;
            }
        }
        #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
        LinkIn(ref for_in_expr) => {
            let res = infer_expr(&for_in_expr.iter, name, decls);
            if res.is_some() {
                return res;
            }
        }
        SumVec4(ref for_n_expr) => return infer_for_n(for_n_expr, name, decls),
        Prod(ref for_n_expr) => return infer_for_n(for_n_expr, name, decls),
        ProdVec4(ref for_n_expr) => return infer_for_n(for_n_expr, name, decls),
        Min(ref for_n_expr) => return infer_for_n(for_n_expr, name, decls),
        Max(ref for_n_expr) => return infer_for_n(for_n_expr, name, decls),
        Sift(ref for_n_expr) => return infer_for_n(for_n_expr, name, decls),
        Any(ref for_n_expr) => return infer_for_n(for_n_expr, name, decls),
        All(ref for_n_expr) => return infer_for_n(for_n_expr, name, decls),
        LinkFor(ref for_n_expr) => return infer_for_n(for_n_expr, name, decls),
        If(ref if_expr) => {
            let res = infer_expr(&if_expr.cond, name, decls);
            if res.is_some() {
                return res;
            }
            let res = infer_block(&if_expr.true_block, name, decls);
            if res.is_some() {
                return res;
            }
            for (cond, block) in if_expr
                .else_if_conds
                .iter()
                .zip(if_expr.else_if_blocks.iter())
            {
                let res = infer_expr(cond, name, decls);
                if res.is_some() {
                    return res;
                }
                let res = infer_block(block, name, decls);
                if res.is_some() {
                    return res;
                }
            }
            if let Some(ref else_block) = if_expr.else_block {
                let res = infer_block(else_block, name, decls);
                if res.is_some() {
                    return res;
                }
            }
        }
        Variable(_) => {}
        Try(ref expr) => {
            let res = infer_expr(expr, name, decls);
            if res.is_some() {
                return res;
            }
        }
        Swizzle(ref swizzle_expr) => {
            let res = infer_expr(&swizzle_expr.expr, name, decls);
            if res.is_some() {
                return res;
            }
        }
        Closure(_) => {}
        CallClosure(ref call) => {
            let res = infer_call_closure(call, name, decls);
            if res.is_some() {
                return res;
            }
        }
        Grab(_) => {}
        TryExpr(ref tr) => {
            let res = infer_expr(&tr.expr, name, decls);
            if res.is_some() {
                return res;
            }
        }
        In(_) => {}
    };
    None
}

fn infer_item(item: &Item, name: &str, decls: &mut Vec<Arc<String>>) -> Option<Item> {
    if item.ids.is_empty() {
        return None;
    }
    for (i, id) in item.ids.iter().enumerate() {
        if let Id::Expression(ref expr) = *id {
            if let Expression::Item(ref id) = *expr {
                if &**id.name == name {
                    return Some(item.trunc(i));
                } else {
                    for decl in decls.iter().rev() {
                        if **decl == **id.name {
                            // It was declared after the index we look for,
                            // so it is not valid.
                            return None;
                        }
                    }
                    let res = infer_expr(expr, name, decls);
                    if res.is_some() {
                        return res;
                    }
                }
            } else {
                // Try infer from expression inside id.
                let res = infer_expr(expr, name, decls);
                if res.is_some() {
                    return res;
                }
                break;
            }
        }
    }
    None
}

fn infer_call(call: &Call, name: &str, decls: &mut Vec<Arc<String>>) -> Option<Item> {
    for arg in &call.args {
        let res = infer_expr(arg, name, decls);
        if res.is_some() {
            return res;
        }
    }
    None
}

fn infer_call_closure(
    call: &CallClosure,
    name: &str,
    decls: &mut Vec<Arc<String>>,
) -> Option<Item> {
    let res = infer_item(&call.item, name, decls);
    if res.is_some() {
        return res;
    }
    for arg in &call.args {
        let res = infer_expr(arg, name, decls);
        if res.is_some() {
            return res;
        }
    }
    None
}

fn infer_for_n(for_n_expr: &ForN, name: &str, decls: &mut Vec<Arc<String>>) -> Option<Item> {
    // Check for declaration of same name.
    if &**for_n_expr.name == name {
        return None;
    } else {
        decls.push(for_n_expr.name.clone());
    }
    let f = |decls: &mut Vec<Arc<String>>| -> Option<Item> {
        if let Some(ref start) = for_n_expr.start {
            let res = infer_expr(start, name, decls);
            if res.is_some() {
                return res;
            }
        }
        let res = infer_expr(&for_n_expr.end, name, decls);
        if res.is_some() {
            return res;
        }
        let res = infer_block(&for_n_expr.block, name, decls);
        if res.is_some() {
            return res;
        }
        None
    };
    let st = decls.len();
    let res = { f(decls) };
    decls.truncate(st);
    res
}

fn infer_block(block: &Block, name: &str, decls: &mut Vec<Arc<String>>) -> Option<Item> {
    let f = |decls: &mut Vec<Arc<String>>| -> Option<Item> {
        for expr in &block.expressions {
            if let Expression::Assign(ref assign_expr) = *expr {
                // Check right expression before left expression.
                let right = infer_expr(&assign_expr.right, name, decls);
                if right.is_some() {
                    return right;
                }
                // Check for declaration of same name.
                if let Expression::Item(ref item) = assign_expr.left {
                    if &**item.name == name {
                        return None;
                    } else if item.ids.is_empty() && assign_expr.op == AssignOp::Assign {
                        decls.push(item.name.clone());
                    }
                }
                let left = infer_expr(&assign_expr.left, name, decls);
                if left.is_some() {
                    return left;
                }
            } else {
                let res = infer_expr(expr, name, decls);
                if res.is_some() {
                    return res;
                }
            }
        }
        None
    };
    let st = decls.len();
    let res = { f(decls) };
    decls.truncate(st);
    res
}
