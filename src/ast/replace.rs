use std::sync::Arc;

use Variable;
use super::{
    Array,
    ArrayFill,
    Assign,
    Block,
    BinOpExpression,
    Call,
    CallClosure,
    Compare,
    Expression,
    For,
    ForN,
    ForIn,
    Go,
    Id,
    If,
    Item,
    Link,
    Object,
    Norm,
    Swizzle,
    UnOpExpression,
    Vec4,
    Mat4,
    TryExpr,
};

/// Replaces an item with a number.
/// Returns `(true, new_expression)` if item found declared with same name.
/// Returns `(false, cloned_expression)` if there was no item with same name.
/// The flag is used to just clone the rest of expressions in a block.
pub fn number(expr: &Expression, name: &Arc<String>, val: f64) -> Expression {
    use super::Expression as E;

    match *expr {
        E::Link(ref link_expr) => {
            let mut new_items: Vec<Expression> = vec![];
            for item in &link_expr.items {
                new_items.push(number(item, name, val));
            }
            E::Link(Box::new(Link {
                items: new_items,
                source_range: link_expr.source_range,
            }))
        }
        E::BinOp(ref bin_op_expr) => {
            E::BinOp(Box::new(BinOpExpression {
                op: bin_op_expr.op,
                left: number(&bin_op_expr.left, name, val),
                right: number(&bin_op_expr.right, name, val),
                source_range: bin_op_expr.source_range,
            }))
        }
        E::Item(ref item) => {
            if &item.name == name {
                E::Variable(Box::new((item.source_range, Variable::f64(val))))
            } else {
                let mut new_ids: Vec<Id> = vec![];
                for id in &item.ids {
                    if let &Id::Expression(ref expr) = id {
                        new_ids.push(Id::Expression(number(expr, name, val)));
                    } else {
                        new_ids.push(id.clone());
                    }
                }
                E::Item(Box::new(Item {
                    name: item.name.clone(),
                    current: item.current,
                    stack_id: item.stack_id.clone(),
                    static_stack_id: item.static_stack_id.clone(),
                    try: item.try.clone(),
                    ids: new_ids,
                    try_ids: item.try_ids.clone(),
                    source_range: item.source_range,
                }))
            }
        }
        E::Block(ref block) => {
            E::Block(Box::new(number_block(block, name, val)))
        }
        E::Assign(ref assign_expr) => {
            E::Assign(Box::new(Assign {
                op: assign_expr.op.clone(),
                left: number(&assign_expr.left, name, val),
                right: number(&assign_expr.right, name, val),
                source_range: assign_expr.source_range,
            }))
        }
        E::Object(ref obj_expr) => {
            let mut new_key_values: Vec<(Arc<String>, Expression)> = vec![];
            for key_value in &obj_expr.key_values {
                new_key_values.push((key_value.0.clone(),
                    number(&key_value.1, name, val)));
            }
            E::Object(Box::new(Object {
                key_values: new_key_values,
                source_range: obj_expr.source_range,
            }))
        }
        E::Call(ref call_expr) => {
            E::Call(Box::new(number_call(call_expr, name, val)))
        }
        E::Array(ref array_expr) => {
            let mut new_items: Vec<Expression> = vec![];
            for item in &array_expr.items {
                new_items.push(number(item, name, val));
            }
            E::Array(Box::new(Array {
                items: new_items,
                source_range: array_expr.source_range,
            }))
        }
        E::ArrayFill(ref array_fill_expr) => {
            E::ArrayFill(Box::new(ArrayFill {
                fill: number(&array_fill_expr.fill, name, val),
                n: number(&array_fill_expr.n, name, val),
                source_range: array_fill_expr.source_range,
            }))
        }
        E::Return(ref ret_expr) => {
            E::Return(Box::new(number(ret_expr, name, val)))
        }
        E::ReturnVoid(_) => expr.clone(),
        E::Break(_) => expr.clone(),
        E::Continue(_) => expr.clone(),
        E::Go(ref go) => {
            E::Go(Box::new(Go {
                call: number_call(&go.call, name, val),
                source_range: go.source_range,
            }))
        }
        E::Vec4(ref vec4_expr) => {
            let mut new_args: Vec<Expression> = vec![];
            for arg in &vec4_expr.args {
                new_args.push(number(arg, name, val));
            }
            E::Vec4(Box::new(Vec4 {
                args: new_args,
                source_range: vec4_expr.source_range,
            }))
        }
        E::Mat4(ref mat4_expr) => {
            let mut new_args: Vec<Expression> = vec![];
            for arg in &mat4_expr.args {
                new_args.push(number(arg, name, val));
            }
            E::Mat4(Box::new(Mat4 {
                args: new_args,
                source_range: mat4_expr.source_range,
            }))
        }
        E::For(ref for_expr) => {
            let mut init: Option<Expression> = None;
            if let Expression::Assign(ref assign_expr) = for_expr.init {
                // Check for declaration of same name.
                if let Expression::Item(ref item) = assign_expr.left {
                    if &item.name == name {
                        init = Some(Expression::Assign(Box::new(Assign {
                            op: assign_expr.op.clone(),
                            left: assign_expr.left.clone(),
                            right: number(&assign_expr.right, name, val),
                            source_range: assign_expr.source_range,
                        })));
                    }
                }
            }
            if let Some(init) = init {
                E::For(Box::new(For {
                    label: for_expr.label.clone(),
                    init: init,
                    cond: for_expr.cond.clone(),
                    step: for_expr.step.clone(),
                    block: for_expr.block.clone(),
                    source_range: for_expr.source_range,
                }))
            } else {
                E::For(Box::new(For {
                    label: for_expr.label.clone(),
                    init: number(&for_expr.init, name, val),
                    cond: number(&for_expr.cond, name, val),
                    step: number(&for_expr.step, name, val),
                    block: number_block(&for_expr.block, name, val),
                    source_range: for_expr.source_range,
                }))
            }
        }
        E::ForIn(ref for_in_expr) => {
            E::ForIn(Box::new(ForIn {
                label: for_in_expr.label.clone(),
                name: for_in_expr.name.clone(),
                iter: number(&for_in_expr.iter, name, val),
                block: number_block(&for_in_expr.block, name, val),
                source_range: for_in_expr.source_range,
            }))
        }
        E::SumIn(ref for_in_expr) => {
            E::SumIn(Box::new(ForIn {
                label: for_in_expr.label.clone(),
                name: for_in_expr.name.clone(),
                iter: number(&for_in_expr.iter, name, val),
                block: number_block(&for_in_expr.block, name, val),
                source_range: for_in_expr.source_range,
            }))
        }
        E::ProdIn(ref for_in_expr) => {
            E::ProdIn(Box::new(ForIn {
                label: for_in_expr.label.clone(),
                name: for_in_expr.name.clone(),
                iter: number(&for_in_expr.iter, name, val),
                block: number_block(&for_in_expr.block, name, val),
                source_range: for_in_expr.source_range,
            }))
        }
        E::MinIn(ref for_in_expr) => {
            E::MinIn(Box::new(ForIn {
                label: for_in_expr.label.clone(),
                name: for_in_expr.name.clone(),
                iter: number(&for_in_expr.iter, name, val),
                block: number_block(&for_in_expr.block, name, val),
                source_range: for_in_expr.source_range,
            }))
        }
        E::MaxIn(ref for_in_expr) => {
            E::MaxIn(Box::new(ForIn {
                label: for_in_expr.label.clone(),
                name: for_in_expr.name.clone(),
                iter: number(&for_in_expr.iter, name, val),
                block: number_block(&for_in_expr.block, name, val),
                source_range: for_in_expr.source_range,
            }))
        }
        E::AnyIn(ref for_in_expr) => {
            E::AnyIn(Box::new(ForIn {
                label: for_in_expr.label.clone(),
                name: for_in_expr.name.clone(),
                iter: number(&for_in_expr.iter, name, val),
                block: number_block(&for_in_expr.block, name, val),
                source_range: for_in_expr.source_range,
            }))
        }
        E::AllIn(ref for_in_expr) => {
            E::AllIn(Box::new(ForIn {
                label: for_in_expr.label.clone(),
                name: for_in_expr.name.clone(),
                iter: number(&for_in_expr.iter, name, val),
                block: number_block(&for_in_expr.block, name, val),
                source_range: for_in_expr.source_range,
            }))
        }
        E::SiftIn(ref for_in_expr) => {
            E::SiftIn(Box::new(ForIn {
                label: for_in_expr.label.clone(),
                name: for_in_expr.name.clone(),
                iter: number(&for_in_expr.iter, name, val),
                block: number_block(&for_in_expr.block, name, val),
                source_range: for_in_expr.source_range,
            }))
        }
        E::LinkIn(ref for_in_expr) => {
            E::LinkIn(Box::new(ForIn {
                label: for_in_expr.label.clone(),
                name: for_in_expr.name.clone(),
                iter: number(&for_in_expr.iter, name, val),
                block: number_block(&for_in_expr.block, name, val),
                source_range: for_in_expr.source_range,
            }))
        }
        E::ForN(ref for_n_expr) => {
            E::ForN(Box::new(number_for_n(for_n_expr, name, val)))
        }
        E::Sum(ref for_n_expr) => {
            E::Sum(Box::new(number_for_n(for_n_expr, name, val)))
        }
        E::SumVec4(ref for_n_expr) => {
            E::SumVec4(Box::new(number_for_n(for_n_expr, name, val)))
        }
        E::Prod(ref for_n_expr) => {
            E::Prod(Box::new(number_for_n(for_n_expr, name, val)))
        }
        E::ProdVec4(ref for_n_expr) => {
            E::ProdVec4(Box::new(number_for_n(for_n_expr, name, val)))
        }
        E::Min(ref for_n_expr) => {
            E::Min(Box::new(number_for_n(for_n_expr, name, val)))
        }
        E::Max(ref for_n_expr) => {
            E::Max(Box::new(number_for_n(for_n_expr, name, val)))
        }
        E::Sift(ref for_n_expr) => {
            E::Sift(Box::new(number_for_n(for_n_expr, name, val)))
        }
        E::Any(ref for_n_expr) => {
            E::Any(Box::new(number_for_n(for_n_expr, name, val)))
        }
        E::All(ref for_n_expr) => {
            E::All(Box::new(number_for_n(for_n_expr, name, val)))
        }
        E::LinkFor(ref for_n_expr) => {
            E::LinkFor(Box::new(number_for_n(for_n_expr, name, val)))
        }
        E::If(ref if_expr) => {
            let mut new_else_if_conds: Vec<Expression> = vec![];
            for else_if_cond in &if_expr.else_if_conds {
                new_else_if_conds.push(number(else_if_cond, name, val));
            }
            let mut new_else_if_blocks: Vec<Block> = vec![];
            for else_if_block in &if_expr.else_if_blocks {
                new_else_if_blocks.push(number_block(else_if_block, name, val));
            }
            E::If(Box::new(If {
                cond: number(&if_expr.cond, name, val),
                true_block: number_block(&if_expr.true_block, name, val),
                else_if_conds: new_else_if_conds,
                else_if_blocks: new_else_if_blocks,
                else_block: if_expr.else_block.as_ref()
                    .map(|else_block| number_block(else_block, name, val)),
                source_range: if_expr.source_range,
            }))
        }
        E::Compare(ref cmp_expr) => {
            E::Compare(Box::new(Compare {
                op: cmp_expr.op.clone(),
                left: number(&cmp_expr.left, name, val),
                right: number(&cmp_expr.right, name, val),
                source_range: cmp_expr.source_range,
            }))
        }
        E::Norm(ref norm) => {
            E::Norm(Box::new(Norm {
                expr: number(&norm.expr, name, val),
                source_range: norm.source_range,
            }))
        }
        E::UnOp(ref unop_expr) => {
            E::UnOp(Box::new(UnOpExpression {
                op: unop_expr.op.clone(),
                expr: number(&unop_expr.expr, name, val),
                source_range: unop_expr.source_range,
            }))
        }
        E::Variable(_) => expr.clone(),
        E::Try(ref expr) => E::Try(Box::new(number(expr, name, val))),
        E::Swizzle(ref swizzle_expr) => {
            E::Swizzle(Box::new(Swizzle {
                sw0: swizzle_expr.sw0.clone(),
                sw1: swizzle_expr.sw1.clone(),
                sw2: swizzle_expr.sw2.clone(),
                sw3: swizzle_expr.sw3.clone(),
                expr: number(&swizzle_expr.expr, name, val),
                source_range: swizzle_expr.source_range,
            }))
        }
        E::Closure(_) => expr.clone(),
        E::CallClosure(ref call_expr) => {
            E::CallClosure(Box::new(number_call_closure(call_expr, name, val)))
        }
        E::Grab(_) => expr.clone(),
        E::TryExpr(ref try_expr) => E::TryExpr(Box::new(TryExpr {
            expr: number(&try_expr.expr, name, val),
            source_range: try_expr.source_range
        })),
        E::In(_) => expr.clone(),
    }
}

fn number_call(call_expr: &Call, name: &Arc<String>, val: f64) -> Call {
    let mut new_args: Vec<Expression> = vec![];
    for arg in &call_expr.args {
        new_args.push(number(arg, name, val));
    }
    Call {
        alias: call_expr.alias.clone(),
        name: call_expr.name.clone(),
        args: new_args,
        f_index: call_expr.f_index.clone(),
        custom_source: None,
        source_range: call_expr.source_range,
    }
}

fn number_call_closure(call_expr: &CallClosure, name: &Arc<String>, val: f64) -> CallClosure {
    let mut new_args: Vec<Expression> = vec![];
    for arg in &call_expr.args {
        new_args.push(number(arg, name, val));
    }
    CallClosure {
        item: call_expr.item.clone(),
        args: new_args,
        source_range: call_expr.source_range,
    }
}

fn number_block(block: &Block, name: &Arc<String>, val: f64) -> Block {
    let mut new_expressions: Vec<Expression> = vec![];
    let mut just_clone = false;
    for expr in &block.expressions {
        if just_clone {
            new_expressions.push(expr.clone());
        } else {
            if let &Expression::Assign(ref assign_expr) = expr {
                // Check for declaration of same name.
                if let Expression::Item(ref item) = assign_expr.left {
                    if &item.name == name {
                        new_expressions.push(Expression::Assign(Box::new(Assign {
                            op: assign_expr.op.clone(),
                            left: assign_expr.left.clone(),
                            right: number(&assign_expr.right, name, val),
                            source_range: assign_expr.source_range,
                        })));
                        just_clone = true;
                        continue;
                    }
                }
            }
            new_expressions.push(number(expr, name, val));
        }
    }
    Block {
        expressions: new_expressions,
        source_range: block.source_range,
    }
}

fn number_for_n(for_n_expr: &ForN, name: &Arc<String>, val: f64) -> ForN {
    if &for_n_expr.name == name {
        for_n_expr.clone()
    } else {
        ForN {
            label: for_n_expr.label.clone(),
            name: for_n_expr.name.clone(),
            start: for_n_expr.start.as_ref()
                .map(|start| number(start, name, val)),
            end: number(&for_n_expr.end, name, val),
            block: number_block(&for_n_expr.block, name, val),
            source_range: for_n_expr.source_range,
        }
    }
}
