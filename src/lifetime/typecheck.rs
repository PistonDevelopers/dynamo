use super::kind::Kind;
use super::node::Node;
use ast::UseLookup;
use range::Range;
use Prelude;
use Type;

mod refine;

/// Runs type checking.
///
/// The type checking consists of 2 steps:
///
/// 1. Propagate types across graph nodes, check for direct conflicts
/// 2. After type propagation, check for missing or conflicting types
///
/// ### Step 1 - Propagate types
///
/// Step 1 runs as long any type information is propagated in the graph.
/// It stops when no further type information can be inferred.
///
/// This step is necessary to infer types of expressions, and can be quite complex.
///
/// When a node gets type information, it will no longer be checked.
/// Therefore, some nodes might delay setting a type to itself even it is known
/// because the node serves as a propagation point for other nodes.
///
/// For example, when declaring a local variable:
///
/// ```ignore
/// x := 2 + a
/// ```
///
/// It is known that a declaration always return `void`, but this knowledge is not always used.
/// Since the type of the left argument depends on the right one,
/// the assignment waits with setting type information until the type of the right expression
/// is known. Then it copies the type over to the left expression and then set itself to `void`.
///
/// ### Step 2 - After type propagation
///
/// This step is used to check conflicts between multiple ways of inferring types.
///
/// For example, the type of an `if` expression is inferred from the true block.
/// The type propagation step uses this assumption without checking the whole `if` expression.
/// After type propagation, all blocks in the `if` expression should have some type information,
/// but no further propagation is necessary, so it only need to check for consistency.
pub(crate) fn run(
    nodes: &mut Vec<Node>,
    prelude: &Prelude,
    use_lookup: &UseLookup,
) -> Result<(), Range<String>> {
    use std::collections::HashMap;

    // Keep an extra todo-list for nodes that are affected by type refinement.
    let mut todo: Vec<usize> = (0..nodes.len()).collect();
    // Keep an extra delay-errors map for nodes that should not report an error after all,
    // if the type refined turned out to match.
    let mut delay_errs: HashMap<usize, Range<String>> = HashMap::new();
    // Type propagation.
    let mut changed;
    loop {
        changed = false;
        // Keep track of the old length of the todo-list,
        // in order to separate tasks already done from new tasks.
        let todo_len = todo.len();
        'node: for i in 0..todo.len() {
            let i = todo[i];
            let kind = nodes[i].kind;
            let mut this_ty = None;
            match kind {
                // No further work required for these statements.
                Kind::Uses | Kind::Start | Kind::End => continue 'node,
                Kind::Go => {
                    // Infer thread type from function.
                    if !nodes[i].children.is_empty() {
                        let ch = nodes[i].children[0];
                        if let Some(ref ty) = nodes[ch].ty {
                            this_ty = Some(Type::Thread(Box::new(ty.clone())))
                        }
                    }
                }
                Kind::Fn => {
                    if nodes[i].ty.is_some() {
                        // No further work is required for function.
                        continue 'node;
                    }
                    if let Some(ch) = nodes[i].find_child_by_kind(nodes, Kind::Expr) {
                        // If the block is unreachable at the end,
                        // this does not tell anything about the type of the function.
                        if nodes[ch].ty == Some(Type::Unreachable) {
                            todo.push(i);
                            continue 'node;
                        }
                        // Infer return type from body of function.
                        this_ty = nodes[ch].ty.clone();
                    }
                }
                Kind::CallArg => {
                    if nodes[i].children.is_empty() || nodes[i].item_ids() {
                        todo.push(i);
                        continue 'node;
                    }
                    let ch = nodes[i].children[0];
                    let expr_type = nodes[ch].ty.as_ref().map(|ty| nodes[i].inner_type(ty));
                    if let Some(parent) = nodes[i].parent {
                        // Remove previous delay errors.
                        if delay_errs.contains_key(&i) {
                            delay_errs.remove(&i);
                        }

                        // Take into account swizzling for the declared argument position.
                        let j = {
                            let mut sum = 0;
                            'inner: for &p_ch in &nodes[parent].children {
                                if p_ch == i {
                                    break 'inner;
                                }
                                if let Some(sw) =
                                    nodes[p_ch].find_child_by_kind(nodes, Kind::Swizzle)
                                {
                                    for &sw_ch in &nodes[sw].children {
                                        match nodes[sw_ch].kind {
                                            Kind::Sw0 | Kind::Sw1 | Kind::Sw2 | Kind::Sw3 => {
                                                sum += 1;
                                            }
                                            _ => {}
                                        }
                                    }
                                } else {
                                    sum += 1;
                                }
                            }
                            sum
                        };

                        // Check type against all arguments covered by swizzle.
                        let js = if nodes[ch].kind == Kind::Swizzle {
                            let mut sum = 0;
                            for &sw_ch in &nodes[ch].children {
                                match nodes[sw_ch].kind {
                                    Kind::Sw0 | Kind::Sw1 | Kind::Sw2 | Kind::Sw3 => {
                                        sum += 1;
                                    }
                                    _ => {}
                                }
                            }

                            let mut js = vec![];
                            for i in 0..sum {
                                js.push(j + i)
                            }
                            js
                        } else {
                            vec![j]
                        };

                        'inner2: for &j in &js {
                            if nodes[parent].kind == Kind::CallClosure {
                                // TODO: Check argument type against closure.
                                continue 'inner2;
                            }
                            if let Some(decl) = nodes[parent].declaration {
                                let arg = nodes[decl].children[j];
                                match (&expr_type, &nodes[arg].ty) {
                                    (&Some(ref ch_ty), &Some(ref arg_ty)) => {
                                        if !arg_ty.goes_with(ch_ty) {
                                            if !delay_errs.contains_key(&i) {
                                                delay_errs.insert(
                                                    i,
                                                    nodes[i].source.wrap(format!(
                                                        "Type mismatch (#100):\n\
                                                        Expected `{}`, found `{}`",
                                                        arg_ty.description(),
                                                        ch_ty.description()
                                                    )),
                                                );
                                            }
                                            todo.push(i);
                                            continue 'node;
                                        }
                                    }
                                    (&None, _) | (_, &None) => {}
                                }
                            } else if let Some(ref alias) = nodes[parent].alias {
                                use ast::FnAlias;

                                // External functions are treated as loaded in prelude.
                                if let Some(&FnAlias::Loaded(f)) = use_lookup
                                    .aliases
                                    .get(alias)
                                    .and_then(|map| map.get(nodes[parent].name().unwrap()))
                                {
                                    let f = &prelude.list[f];
                                    if let Some(ref ty) = expr_type {
                                        if !f.tys[j].goes_with(ty) {
                                            if !delay_errs.contains_key(&i) {
                                                delay_errs.insert(
                                                    i,
                                                    nodes[i].source.wrap(format!(
                                                        "Type mismatch (#150):\n\
                                                        Expected `{}`, found `{}`",
                                                        f.tys[j].description(),
                                                        ty.description()
                                                    )),
                                                );
                                            }
                                            todo.push(i);
                                            continue 'node;
                                        }
                                    }
                                }
                            } else if let Some(&f) =
                                prelude.functions.get(nodes[parent].name().unwrap())
                            {
                                let f = &prelude.list[f];
                                if let Some(ref ty) = expr_type {
                                    if !f.tys[j].goes_with(ty) {
                                        if !delay_errs.contains_key(&i) {
                                            delay_errs.insert(
                                                i,
                                                nodes[i].source.wrap(format!(
                                                    "Type mismatch (#200):\n\
                                                    Expected `{}`, found `{}`",
                                                    f.tys[j].description(),
                                                    ty.description()
                                                )),
                                            );
                                        }
                                        todo.push(i);
                                        continue 'node;
                                    }
                                }
                            }
                        }
                    }
                    this_ty = expr_type;
                }
                Kind::Call => {
                    if let Some(decl) = nodes[i].declaration {
                        refine::declaration(i, decl, nodes, &mut todo, &mut this_ty)?;

                        // If the type has not been refined, fall back to default type signature.
                        if this_ty.is_none() && nodes[i].ty.is_none() {
                            if let Some(ref ty) = nodes[decl].ty {
                                this_ty = Some(ty.clone());
                            }
                        }
                    } else if let Some(ref alias) = nodes[i].alias {
                        use ast::FnAlias;

                        // External functions are treated as loaded in prelude.
                        if let Some(&FnAlias::Loaded(f)) = use_lookup
                            .aliases
                            .get(alias)
                            .and_then(|map| map.get(nodes[i].name().unwrap()))
                        {
                            let f = &prelude.list[f];
                            if f.ext.is_empty() {
                                this_ty = Some(f.ret.clone());
                            } else {
                                refine::prelude(i, f, nodes, &mut todo, &mut this_ty)?;

                                // If the type has not been refined, fall back to default type signature.
                                if this_ty.is_none() && nodes[i].ty.is_none() {
                                    this_ty = Some(f.ret.clone());
                                }
                            }
                        }
                    } else if let Some(&f) = prelude.functions.get(nodes[i].name().unwrap()) {
                        let f = &prelude.list[f];
                        if f.ext.is_empty() {
                            this_ty = Some(f.ret.clone());
                        } else {
                            refine::prelude(i, f, nodes, &mut todo, &mut this_ty)?;

                            // If the type has not been refined, fall back to default type signature.
                            if this_ty.is_none() && nodes[i].ty.is_none() {
                                this_ty = Some(f.ret.clone());
                            }
                        }
                    }
                }
                Kind::CallClosure => {
                    if let Some(item) = nodes[i].find_child_by_kind(nodes, Kind::Item) {
                        if nodes[item].item_ids() {
                            todo.push(i);
                            continue 'node;
                        }
                        if let Some(decl) = nodes[item].declaration {
                            if let Some(ref ty) = nodes[decl].ty {
                                if let Some(ty) = ty.closure_ret_ty() {
                                    this_ty = Some(ty);
                                } else {
                                    return Err(nodes[item].source.wrap(format!(
                                        "Type mismatch (#250):\n\
                                                Expected `closure`, found `{}`",
                                        ty.description()
                                    )));
                                }
                            }
                        }
                    }
                }
                Kind::Assign => {
                    let left = match nodes[i].find_child_by_kind(nodes, Kind::Left) {
                        None => {
                            todo.push(i);
                            continue 'node;
                        }
                        Some(x) => x,
                    };
                    let right = match nodes[i].find_child_by_kind(nodes, Kind::Right) {
                        None => {
                            todo.push(i);
                            continue 'node;
                        }
                        Some(x) => x,
                    };
                    if nodes[right].item_ids() {
                        todo.push(i);
                        continue 'node;
                    }
                    nodes[left].ty = match (&nodes[left].ty, &nodes[right].ty) {
                        (&None, &Some(ref right_ty)) => {
                            // Make assign return void since there is no more need for checking.
                            this_ty = Some(Type::Void);
                            Some(right_ty.clone())
                        }
                        (&Some(ref left_ty), &Some(ref right_ty)) => {
                            if right_ty.goes_with(left_ty) {
                                if !nodes[left].children.is_empty() {
                                    // Tell the item that it needs refinement.
                                    let it = nodes[left].children[0];
                                    todo.push(it);
                                    // Tell all nodes that uses the item as declaration that
                                    // they need refinement.
                                    for j in it + 1..nodes.len() {
                                        if let Some(decl) = nodes[j].declaration {
                                            if decl == it {
                                                todo.push(j)
                                            }
                                        }
                                    }
                                }
                                this_ty = Some(Type::Void);
                                Some(right_ty.clone())
                            } else {
                                // TODO: Type conflict between left and refined right.
                                //       Might be caught by later rules.
                                todo.push(i);
                                continue 'node;
                            }
                        }
                        _ => {
                            todo.push(i);
                            continue 'node;
                        }
                    };
                    changed = true;
                }
                Kind::Item => {
                    if nodes[i].item_ids() {
                        todo.push(i);
                        continue 'node;
                    }
                    if let Some(decl) = nodes[i].declaration {
                        match nodes[decl].kind {
                            Kind::Sum
                            | Kind::Min
                            | Kind::Max
                            | Kind::Any
                            | Kind::All
                            | Kind::Sift
                            | Kind::Vec4UnLoop
                            | Kind::ForN
                            | Kind::LinkFor => {
                                if nodes[i].try {
                                    return Err(nodes[i].source.wrap(
                                        "Type mismatch (#300):\n\
                                        Can not use `?` with a number"
                                            .into(),
                                    ));
                                }
                                // All indices are numbers.
                                this_ty = Some(Type::F64);
                            }
                            Kind::Arg => {
                                this_ty = Some(
                                    nodes[i]
                                        .inner_type(nodes[decl].ty.as_ref().unwrap_or(&Type::Any)),
                                );
                            }
                            _ => {
                                if let Some(ref ty) = nodes[decl].ty {
                                    this_ty = Some(nodes[i].inner_type(ty));
                                }
                                if this_ty.is_some() {
                                    // Change the type of left expression,
                                    // to get a more accurate type.
                                    if let Some(parent) = nodes[i].parent {
                                        if nodes[parent].kind == Kind::Left {
                                            nodes[parent].ty = this_ty.clone();
                                        }
                                    }
                                }
                            }
                        }
                    } else if let Some(parent) = nodes[i].parent {
                        if nodes[parent].kind == Kind::Left {
                            if let Some(ref ty) = nodes[parent].ty {
                                // Get type from assignment left expression.
                                this_ty = Some(ty.clone());
                            }
                        }
                    }
                }
                Kind::Return
                | Kind::Val
                | Kind::Expr
                | Kind::Cond
                | Kind::Left
                | Kind::Right
                | Kind::ElseIfCond
                | Kind::Grab
                | Kind::Add
                | Kind::Mul
                | Kind::Pow => {
                    if nodes[i].children.is_empty() {
                        // No further work is required.
                        continue 'node;
                    }
                    let ch = nodes[i].children[0];
                    if nodes[ch].item_ids() {
                        todo.push(i);
                        continue 'node;
                    }
                    let ty = match nodes[ch].ty {
                        None => {
                            todo.push(i);
                            continue 'node;
                        }
                        Some(ref ty) => ty.clone(),
                    };
                    if nodes[i].kind == Kind::Grab && ty == Type::Void {
                        return Err(nodes[i].source.wrap(
                            "Type mismatch (#325):\n\
                                Expected something, found `void`"
                                .to_string(),
                        ));
                    }
                    if nodes[ch].kind == Kind::Return {
                        // Find function and check return type.
                        let mut p = i;
                        loop {
                            p = match nodes[p].parent {
                                None => break,
                                Some(p) => p,
                            };
                            if nodes[p].kind == Kind::Fn || nodes[p].kind == Kind::Closure {
                                if nodes[p].ty.is_none() {
                                    // Infer return type of function.
                                    nodes[p].ty = Some(ty.clone());
                                } else if let Some(ref fn_ty) = nodes[p].ty {
                                    if !fn_ty.goes_with(&ty) {
                                        return Err(nodes[ch].source.wrap(format!(
                                            "Type mismatch (#350):\n\
                                            Expected `{}`, found `{}`",
                                            fn_ty.description(),
                                            ty.description()
                                        )));
                                    }
                                }
                                break;
                            }
                        }
                        this_ty = Some(Type::Unreachable);
                    } else {
                        // Propagate type.
                        this_ty = Some(nodes[i].inner_type(&ty));
                    }
                }
                Kind::Compare => {
                    let left = match nodes[i].find_child_by_kind(nodes, Kind::Left) {
                        None => {
                            todo.push(i);
                            continue 'node;
                        }
                        Some(x) => x,
                    };
                    if nodes[left].item_ids() {
                        todo.push(i);
                        continue 'node;
                    }
                    match nodes[left].ty {
                        Some(Type::Any) => {
                            this_ty = Some(Type::Any);
                        }
                        Some(Type::Secret(_)) => {
                            this_ty = Some(Type::Secret(Box::new(Type::Bool)));
                        }
                        Some(_) => {
                            this_ty = Some(Type::Bool);
                        }
                        _ => {}
                    }
                }
                Kind::Block | Kind::TrueBlock | Kind::ElseIfBlock | Kind::ElseBlock => {
                    if nodes[i].children.is_empty() {
                        this_ty = Some(Type::Void);
                    }
                    if let Some(&ch) = nodes[i].children.last() {
                        if nodes[ch].item_ids() {
                            todo.push(i);
                            continue 'node;
                        }
                        if let Some(ref ty) = nodes[ch].ty {
                            this_ty = Some(nodes[i].inner_type(ty));
                        }
                    }
                }
                Kind::Sift => {
                    // Infer type from body.
                    let ch = if let Some(ch) = nodes[i].find_child_by_kind(nodes, Kind::Block) {
                        ch
                    } else {
                        todo.push(i);
                        continue 'node;
                    };
                    if let Some(ref ty) = nodes[ch].ty {
                        this_ty = Some(Type::Array(Box::new(ty.clone())));
                    }
                }
                Kind::X | Kind::Y | Kind::Z | Kind::W | Kind::N => {
                    if nodes[i].children.is_empty() {
                        todo.push(i);
                        continue 'node;
                    }
                    let ch = nodes[i].children[0];
                    if nodes[ch].item_ids() {
                        todo.push(i);
                        continue 'node;
                    }

                    let expr_type = nodes[ch].ty.as_ref().map(|ty| nodes[i].inner_type(ty));
                    if let Some(ref ty) = expr_type {
                        if !ty.goes_with(&Type::F64) {
                            return Err(nodes[i].source.wrap(format!(
                                "Type mismatch (#700):\nExpected `f64`, found `{}`",
                                expr_type.as_ref().unwrap().description()
                            )));
                        }
                    }
                    this_ty = expr_type;
                }
                Kind::If => {
                    let tb = match nodes[i].find_child_by_kind(nodes, Kind::TrueBlock) {
                        None => {
                            todo.push(i);
                            continue 'node;
                        }
                        Some(tb) => tb,
                    };
                    let true_type = match nodes[tb].ty.as_ref().map(|ty| nodes[i].inner_type(ty)) {
                        None => {
                            todo.push(i);
                            continue 'node;
                        }
                        Some(true_type) => true_type,
                    };

                    this_ty = Some(true_type);
                }
                Kind::Arg => {
                    if nodes[i].ty.is_none() {
                        this_ty = Some(Type::Any);
                    } else {
                        // No further work needed for this node.
                        continue 'node;
                    }
                }
                Kind::Closure => {
                    let mut lts = vec![];
                    let mut tys = vec![];
                    let mut ret: Option<Type> = None;
                    let mut all_args = true;
                    for &ch in &nodes[i].children {
                        if nodes[ch].kind == Kind::Arg {
                            if let Some(ref ty) = nodes[ch].ty {
                                use Lt;

                                lts.push(Lt::Default);
                                tys.push(ty.clone());
                            } else {
                                all_args = false;
                                break;
                            }
                        }
                        if nodes[ch].kind == Kind::Expr {
                            ret = nodes[ch].ty.clone();
                        }
                    }
                    if all_args && ret.is_some() {
                        use Dfn;

                        this_ty = Some(Type::Closure(Box::new(Dfn {
                            lts,
                            tys,
                            ret: ret.unwrap(),
                            ext: vec![],
                            lazy: crate::LAZY_NO,
                        })));
                    }
                }
                _ => {}
            }
            if this_ty.is_some() {
                if let (&Some(ref old_ty), &Some(ref new_ty)) = (&nodes[i].ty, &this_ty) {
                    if old_ty != new_ty {
                        // If type was refined, propagate changes to parent.
                        if let Some(parent) = nodes[i].parent {
                            // If the type of the parent is not set,
                            // then there is no need to add it to the todo-list,
                            // since it will be covered by the default loop.
                            if nodes[parent].ty.is_some() {
                                todo.push(parent);
                            }
                        }
                    }
                }
                nodes[i].ty = this_ty;
                changed = true;
            } else {
                todo.push(i);
            }
        }

        // Remove old tasks.
        for i in (0..todo_len).rev() {
            todo.swap_remove(i);
        }
        // There must be a change to continue checking,
        // even with type refinement, because the todo-list
        // waits for other changes to happen.
        if !changed {
            break;
        }

        // Prepare todo-list for binary search.
        todo.sort_unstable();
        todo.dedup();
    }

    // Report one delayed error, if any.
    if !delay_errs.is_empty() {
        return Err(delay_errs.values().next().unwrap().clone());
    }

    // After type propagation.
    for i in 0..nodes.len() {
        let kind = nodes[i].kind;
        match kind {
            Kind::Fn => {
                if let Some(ref ty) = nodes[i].ty {
                    // Check inferred type matches the one of the block.
                    // This is used by mathematical expressions where return type is inferred.
                    if let Some(ch) = nodes[i].find_child_by_kind(nodes, Kind::Expr) {
                        if let Some(ref ch_ty) = nodes[ch].ty {
                            if !ty.goes_with(ch_ty) {
                                return Err(nodes[ch].source.wrap(format!(
                                    "Type mismatch (#750):\nExpected `{}`, found `{}`",
                                    ty.description(),
                                    ch_ty.description()
                                )));
                            }
                        }
                    }

                    // Check all return statements.
                    let mut found_return = false;
                    check_fn(i, nodes, ty, &mut found_return)?;
                    // Report if there is no return statement.
                    if !found_return
                        && ty != &Type::Void
                        && nodes[i].find_child_by_kind(nodes, Kind::Expr).is_none()
                    {
                        return Err(nodes[i].source.wrap(format!(
                            "Type mismatch (#775):\nExpected `{}`, found `void`",
                            ty.description()
                        )));
                    }
                } else {
                    return Err(nodes[i].source.wrap(format!(
                        "Type mismatch (#800):\nCould not infer type of function `{}`",
                        nodes[i].name().unwrap()
                    )));
                }
            }
            Kind::Go => {
                if !nodes[i].children.is_empty() {
                    if let Some(decl) = nodes[nodes[i].children[0]].declaration {
                        match nodes[decl].ty {
                            None | Some(Type::Void) => {
                                return Err(nodes[i].source.wrap(format!(
                                    "Type mismatch (#900):\nRequires `->` on `{}`",
                                    nodes[decl].name().unwrap()
                                )));
                            }
                            _ => {}
                        }
                    }
                }
            }
            Kind::If => check_if(i, nodes)?,
            Kind::Assign => {
                use ast::AssignOp;

                match nodes[i].op {
                    Some(AssignOp::Add) | Some(AssignOp::Sub) => {
                        let left = nodes[i].find_child_by_kind(nodes, Kind::Left).unwrap();
                        let right = nodes[i].find_child_by_kind(nodes, Kind::Right).unwrap();
                        if let Some(ref left_ty) = nodes[left].ty {
                            if let Some(ref right_ty) = nodes[right].ty {
                                if !left_ty.add_assign(right_ty) {
                                    return Err(nodes[i].source.wrap(format!(
                                        "Type mismatch (#1000):\n\
                                        Assignment operator can not be used with `{}` and `{}`",
                                        left_ty.description(),
                                        right_ty.description()
                                    )));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Kind::Block => {
                // Make sure all results are used.
                // TODO: If the block is the body of a for loop,
                // then the last child node should be checked too.
                let n = nodes[i].children.len();
                if n == 0 {
                    continue;
                }
                let children = if let Some(parent) = nodes[i].parent {
                    match nodes[parent].kind {
                        Kind::Fn => match nodes[parent].ty {
                            Some(Type::Void) => &nodes[i].children,
                            None => continue,
                            _ => &nodes[i].children[0..n - 1],
                        },
                        _ => &nodes[i].children[0..n - 1],
                    }
                } else {
                    &nodes[i].children[0..n - 1]
                };
                for &ch in children.iter() {
                    if let Kind::Return = nodes[ch].kind {
                        continue;
                    };
                    if let Some(ref ty) = nodes[ch].ty {
                        if ty != &Type::Void && ty != &Type::Unreachable {
                            return Err(nodes[ch].source.wrap(format!(
                                "Type mismatch (#1100):\nUnused result `{}`",
                                ty.description()
                            )));
                        }
                    }
                }
            }
            Kind::Swizzle => {
                if let Some(ch) = nodes[i].find_child_by_kind(nodes, Kind::Expr) {
                    let expr_type = nodes[ch].ty.as_ref().map(|ty| nodes[ch].inner_type(ty));
                    if let Some(ref ty) = expr_type {
                        if !ty.goes_with(&Type::Vec4) {
                            return Err(nodes[ch].source.wrap(format!(
                                "Type mismatch (#1200):\nExpected `vec4`, found `{}`",
                                expr_type.as_ref().unwrap().description()
                            )));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Checks all returns recursively in function.
fn check_fn(
    n: usize,
    nodes: &[Node],
    ty: &Type,
    found_return: &mut bool,
) -> Result<(), Range<String>> {
    for &ch in &nodes[n].children {
        match nodes[ch].kind {
            Kind::Return => {
                if let Some(ref ret_ty) = nodes[ch].ty {
                    if !ty.goes_with(ret_ty) {
                        return Err(nodes[ch].source.wrap(format!(
                            "Type mismatch (#1200):\nExpected `{}`, found `{}`",
                            ty.description(),
                            ret_ty.description()
                        )));
                    }
                }
                *found_return = true;
            }
            Kind::ReturnVoid => {
                if !ty.goes_with(&Type::Void) {
                    return Err(nodes[ch].source.wrap(format!(
                        "Type mismatch (#1300):\nExpected `{}`, found `{}`",
                        ty.description(),
                        Type::Void.description()
                    )));
                }
                *found_return = true;
            }
            Kind::Item => {
                if nodes[ch].name().as_ref().map(|n| &***n == "return") == Some(true) {
                    if let Some(parent) = nodes[ch].parent {
                        if nodes[parent].kind == Kind::Left {
                            if let Some(parent) = nodes[parent].parent {
                                if nodes[parent].kind == Kind::Assign {
                                    if let Some(ref ret_ty) = nodes[ch].ty {
                                        if !ty.goes_with(ret_ty) {
                                            return Err(nodes[ch].source.wrap(format!(
                                                "Type mismatch (#1250):\nExpected `{}`, found `{}`",
                                                ty.description(),
                                                ret_ty.description()
                                            )));
                                        }
                                    }
                                    *found_return = true;
                                }
                            }
                        }
                    }
                }
            }
            Kind::Closure => {
                continue;
            }
            _ => {}
        }
        check_fn(ch, nodes, ty, found_return)?;
    }
    Ok(())
}

fn check_if(n: usize, nodes: &[Node]) -> Result<(), Range<String>> {
    if let Some(ch) = nodes[n].find_child_by_kind(nodes, Kind::Cond) {
        if let Some(ref cond_ty) = nodes[ch].ty {
            if !Type::Bool.goes_with(cond_ty) {
                return Err(nodes[ch].source.wrap(format!(
                    "Type mismatch (#1400):\nExpected `{}`, found `{}`",
                    Type::Bool.description(),
                    cond_ty.description()
                )));
            }
        }
    }

    // The type of ifs are inferred from the true block.
    let true_type = match nodes[n].ty {
        None => return Ok(()),
        Some(ref ty) => ty,
    };

    for &ch in &nodes[n].children {
        if let Kind::ElseIfCond = nodes[ch].kind {
            if let Some(ref cond_ty) = nodes[ch].ty {
                if !Type::Bool.goes_with(cond_ty) {
                    return Err(nodes[ch].source.wrap(format!(
                        "Type mismatch (#1500):\nExpected `{}`, found `{}`",
                        Type::Bool.description(),
                        cond_ty.description()
                    )));
                }
            }
        } else if let Kind::ElseIfBlock = nodes[ch].kind {
            if let Some(ref else_if_type) = nodes[ch].ty {
                if !else_if_type.goes_with(true_type) {
                    return Err(nodes[ch].source.wrap(format!(
                        "Type mismatch (#1600):\nExpected `{}`, found `{}`",
                        true_type.description(),
                        else_if_type.description()
                    )));
                }
            }
        }
    }

    if let Some(eb) = nodes[n].find_child_by_kind(nodes, Kind::ElseBlock) {
        if let Some(ref else_type) = nodes[eb].ty {
            if !else_type.goes_with(true_type) {
                return Err(nodes[eb].source.wrap(format!(
                    "Type mismatch (#1700):\nExpected `{}`, found `{}`",
                    true_type.description(),
                    else_type.description()
                )));
            }
        }
    }

    Ok(())
}
