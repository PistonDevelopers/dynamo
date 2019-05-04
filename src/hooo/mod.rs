//! Higher Order Operator Overloading (HOOO)

use super::*;

use std::sync::Arc;

/// Checks that two closures have the same input type.
pub fn same_input_type(a: &Dfn, b: &Dfn) -> bool {
    if a.tys.len() != b.tys.len() {return false};
    for i in 0..a.tys.len() {
        if !a.tys[i].goes_with(&b.tys[i]) {return false}
    }
    true
}

/// Adds two closures.
pub fn binop(
    function: &ast::Function,
    binop: &ast::BinOpExpression,
    a: &Arc<ast::Closure>,
    a_env: &ClosureEnvironment,
    b: &Arc<ast::Closure>,
    b_env: &ClosureEnvironment,
) -> Result<(Arc<ast::Closure>, Box<ClosureEnvironment>), String> {
    if Arc::ptr_eq(&a_env.module, &b_env.module) &&
       a_env.relative == b_env.relative &&
       a.currents == b.currents {
        // Closure environment matches, can inline expressions.
        Ok((
            Arc::new(ast::Closure {
                args: a.args.clone(),
                currents: a.currents.clone(),
                file: function.file.clone(),
                ret: a.ret.clone(),
                source: function.source.clone(),
                source_range: binop.source_range,
                expr: ast::Expression::BinOp(Box::new(ast::BinOpExpression {
                    left: a.expr.clone(),
                    right: b.expr.clone(),
                    op: binop.op,
                    source_range: binop.source_range,
                }))
            }),
            Box::new(a_env.clone())
        ))
    } else {
        use std::cell::Cell;

        // Closure environment does not match, must grab closures.
        Ok((
            Arc::new(ast::Closure {
                args: a.args.clone(),
                currents: vec![],
                file: function.file.clone(),
                ret: a.ret.clone(),
                source: function.source.clone(),
                source_range: binop.source_range,
                expr: ast::Expression::Block(Box::new(ast::Block {
                    source_range: binop.source_range,
                    expressions: vec![
                        // a := grab a
                        ast::Expression::Assign(Box::new(ast::Assign {
                            source_range: binop.source_range,
                            op: ast::AssignOp::Assign,
                            left: ast::Expression::Item(Box::new(ast::Item {
                                name: Arc::new("arg0".into()),
                                current: false,
                                source_range: binop.source_range,
                                stack_id: Cell::new(None),
                                static_stack_id: Cell::new(None),
                                ids: vec![],
                                try: false,
                                try_ids: vec![],
                            })),
                            right: ast::Expression::Variable(Box::new((
                                binop.source_range,
                                Variable::Closure(a.clone(), Box::new(a_env.clone()))
                            )))
                        })),
                        // b := grab b
                        ast::Expression::Assign(Box::new(ast::Assign {
                            source_range: binop.source_range,
                            op: ast::AssignOp::Assign,
                            left: ast::Expression::Item(Box::new(ast::Item {
                                name: Arc::new("arg1".into()),
                                current: false,
                                source_range: binop.source_range,
                                stack_id: Cell::new(None),
                                static_stack_id: Cell::new(None),
                                ids: vec![],
                                try: false,
                                try_ids: vec![],
                            })),
                            right: ast::Expression::Variable(Box::new((
                                binop.source_range,
                                Variable::Closure(b.clone(), Box::new(b_env.clone()))
                            )))
                        })),
                        // a op b
                        ast::Expression::BinOp(Box::new(ast::BinOpExpression {
                            op: binop.op,
                            source_range: binop.source_range,
                            left: ast::Expression::CallClosure(Box::new(ast::CallClosure {
                                source_range: binop.source_range,
                                item: ast::Item {
                                    name: Arc::new("arg0".into()),
                                    current: false,
                                    source_range: binop.source_range,
                                    stack_id: Cell::new(None),
                                    static_stack_id: Cell::new(Some(2)),
                                    ids: vec![],
                                    try: false,
                                    try_ids: vec![],
                                },
                                args: a.args.iter().map(|arg|
                                    ast::Expression::Item(Box::new(ast::Item {
                                        name: arg.name.clone(),
                                        current: false,
                                        source_range: binop.source_range,
                                        stack_id: Cell::new(None),
                                        static_stack_id: Cell::new(Some(3 + a.args.len())),
                                        ids: vec![],
                                        try: false,
                                        try_ids: vec![],
                                    }))).collect(),
                            })),
                            right: ast::Expression::CallClosure(Box::new(ast::CallClosure {
                                source_range: binop.source_range,
                                item: ast::Item {
                                    name: Arc::new("arg1".into()),
                                    current: false,
                                    source_range: binop.source_range,
                                    stack_id: Cell::new(None),
                                    static_stack_id: Cell::new(Some(1)),
                                    ids: vec![],
                                    try: false,
                                    try_ids: vec![],
                                },
                                args: a.args.iter().map(|arg|
                                    ast::Expression::Item(Box::new(ast::Item {
                                        name: arg.name.clone(),
                                        current: false,
                                        source_range: binop.source_range,
                                        stack_id: Cell::new(None),
                                        static_stack_id: Cell::new(Some(3 + a.args.len())),
                                        ids: vec![],
                                        try: false,
                                        try_ids: vec![],
                                    }))).collect(),
                            })),
                        }))
                    ]
                }))
            }),
            // The new environment does not matter, so just using the same as `a`.
            Box::new(a_env.clone())
        ))
    }
}
