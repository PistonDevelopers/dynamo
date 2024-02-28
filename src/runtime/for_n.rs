use super::*;

macro_rules! start(
    ($rt:ident, $for_n_expr:ident) => {
        if let Some(ref start_expr) = $for_n_expr.start {
            let start = match $rt.expression(start_expr, Side::Right)? {
                (x, Flow::Return) => { return Ok((x, Flow::Return)); }
                (Some(x), Flow::Continue) => x,
                _ => return Err($rt.module.error(start_expr.source_range(),
                    &format!("{}\nExpected number from for start",
                        $rt.stack_trace()), $rt))
            };
            let start = match $rt.resolve(&start) {
                &Variable::F64(val, _) => val,
                x => return Err($rt.module.error(start_expr.source_range(),
                                &$rt.expected(x, "number"), $rt))
            };
            start
        } else { 0.0 }
    };
);

macro_rules! end(
    ($rt:ident, $for_n_expr:ident) => {{
        let end = match $rt.expression(&$for_n_expr.end, Side::Right)? {
            (x, Flow::Return) => { return Ok((x, Flow::Return)); }
            (Some(x), Flow::Continue) => x,
            _ => return Err($rt.module.error($for_n_expr.end.source_range(),
                &format!("{}\nExpected number from for end",
                    $rt.stack_trace()), $rt))
        };
        match $rt.resolve(&end) {
            &Variable::F64(val, _) => val,
            x => return Err($rt.module.error($for_n_expr.end.source_range(),
                            &$rt.expected(x, "number"), $rt))
        }
    }};
);

macro_rules! cond(
    ($rt:ident, $for_n_expr:ident, $st:ident, $end:ident) => {
        match &$rt.stack[$st - 1] {
            &Variable::F64(val, _) => {
                if val < $end {}
                else { break }
                val
            }
            x => return Err($rt.module.error($for_n_expr.source_range,
                            &$rt.expected(x, "number"), $rt))
        }
    };
);

macro_rules! break_(
    ($x:ident, $for_n_expr:ident, $flow:ident) => {{
        if let Some(label) = $x {
            let same =
            if let Some(ref for_label) = $for_n_expr.label {
                &label == for_label
            } else { false };
            if !same {
                $flow = Flow::Break(Some(label))
            }
        }
        break;
    }};
    ($x:ident, $for_n_expr:ident, $flow:ident, $label:tt) => {{
        if let Some(label) = $x {
            let same =
            if let Some(ref for_label) = $for_n_expr.label {
                &label == for_label
            } else { false };
            if !same {
                $flow = Flow::Break(Some(label))
            }
        }
        break $label;
    }};
);

macro_rules! continue_(
    ($x:ident, $for_n_expr:ident, $flow:ident) => {{
        if let Some(label) = $x {
            let same =
            if let Some(ref for_label) = $for_n_expr.label {
                &label == for_label
            } else { false };
            if !same {
                $flow = Flow::ContinueLoop(Some(label));
                break;
            }
        }
    }};
);

macro_rules! inc(
    ($rt:ident, $for_n_expr:ident, $st:ident) => {{
        let error = if let Variable::F64(ref mut val, _) = $rt.stack[$st - 1] {
            *val += 1.0;
            false
        } else { true };
        if error {
            return Err($rt.module.error($for_n_expr.source_range,
                       &$rt.expected(&$rt.stack[$st - 1], "number"), $rt))
        }
    }};
);

impl Runtime {
    pub(crate) fn for_n_expr(
        &mut self,
        for_n_expr: &ast::ForN,
    ) -> Result<(Option<Variable>, Flow), String> {
        let prev_st = self.stack.len();
        let prev_lc = self.local_stack.len();

        let start = start!(self, for_n_expr);
        let end = end!(self, for_n_expr);

        // Initialize counter.
        self.local_stack
            .push((for_n_expr.name.clone(), self.stack.len()));
        self.stack.push(Variable::f64(start));

        let st = self.stack.len();
        let lc = self.local_stack.len();
        let mut flow = Flow::Continue;
        loop {
            cond!(self, for_n_expr, st, end);
            match self.block(&for_n_expr.block)? {
                (x, Flow::Return) => {
                    return Ok((x, Flow::Return));
                }
                (_, Flow::Continue) => {}
                (_, Flow::Break(x)) => break_!(x, for_n_expr, flow),
                (_, Flow::ContinueLoop(x)) => continue_!(x, for_n_expr, flow),
            }
            inc!(self, for_n_expr, st);
            self.stack.truncate(st);
            self.local_stack.truncate(lc);
        }
        self.stack.truncate(prev_st);
        self.local_stack.truncate(prev_lc);
        Ok((None, flow))
    }

    pub(crate) fn sum_n_expr(
        &mut self,
        for_n_expr: &ast::ForN,
    ) -> Result<(Option<Variable>, Flow), String> {
        let prev_st = self.stack.len();
        let prev_lc = self.local_stack.len();
        let mut sum = 0.0;

        let start = start!(self, for_n_expr);
        let end = end!(self, for_n_expr);

        // Initialize counter.
        self.local_stack
            .push((for_n_expr.name.clone(), self.stack.len()));
        self.stack.push(Variable::f64(start));

        let st = self.stack.len();
        let lc = self.local_stack.len();
        let mut flow = Flow::Continue;
        loop {
            cond!(self, for_n_expr, st, end);
            match self.block(&for_n_expr.block)? {
                (Some(x), Flow::Continue) => {
                    match self.resolve(&x) {
                        &Variable::F64(val, _) => sum += val,
                        x => {
                            return Err(self.module.error(
                                for_n_expr.block.source_range,
                                &self.expected(x, "number"),
                                self,
                            ))
                        }
                    };
                }
                (x, Flow::Return) => {
                    return Ok((x, Flow::Return));
                }
                (None, Flow::Continue) => {
                    return Err(self.module.error(
                        for_n_expr.block.source_range,
                        "Expected `number`",
                        self,
                    ))
                }
                (_, Flow::Break(x)) => break_!(x, for_n_expr, flow),
                (_, Flow::ContinueLoop(x)) => continue_!(x, for_n_expr, flow),
            }
            inc!(self, for_n_expr, st);
            self.stack.truncate(st);
            self.local_stack.truncate(lc);
        }
        self.stack.truncate(prev_st);
        self.local_stack.truncate(prev_lc);
        Ok((Some(Variable::f64(sum)), flow))
    }

    pub(crate) fn prod_n_expr(
        &mut self,
        for_n_expr: &ast::ForN,
    ) -> Result<(Option<Variable>, Flow), String> {
        let prev_st = self.stack.len();
        let prev_lc = self.local_stack.len();
        let mut prod = 1.0;

        let start = start!(self, for_n_expr);
        let end = end!(self, for_n_expr);

        // Initialize counter.
        self.local_stack
            .push((for_n_expr.name.clone(), self.stack.len()));
        self.stack.push(Variable::f64(start));

        let st = self.stack.len();
        let lc = self.local_stack.len();
        let mut flow = Flow::Continue;
        loop {
            cond!(self, for_n_expr, st, end);
            match self.block(&for_n_expr.block)? {
                (Some(x), Flow::Continue) => {
                    match self.resolve(&x) {
                        &Variable::F64(val, _) => prod *= val,
                        x => {
                            return Err(self.module.error(
                                for_n_expr.block.source_range,
                                &self.expected(x, "number"),
                                self,
                            ))
                        }
                    };
                }
                (x, Flow::Return) => {
                    return Ok((x, Flow::Return));
                }
                (None, Flow::Continue) => {
                    return Err(self.module.error(
                        for_n_expr.block.source_range,
                        "Expected `number`",
                        self,
                    ))
                }
                (_, Flow::Break(x)) => break_!(x, for_n_expr, flow),
                (_, Flow::ContinueLoop(x)) => continue_!(x, for_n_expr, flow),
            }
            inc!(self, for_n_expr, st);
            self.stack.truncate(st);
            self.local_stack.truncate(lc);
        }
        self.stack.truncate(prev_st);
        self.local_stack.truncate(prev_lc);
        Ok((Some(Variable::f64(prod)), flow))
    }

    pub(crate) fn min_n_expr(
        &mut self,
        for_n_expr: &ast::ForN,
    ) -> Result<(Option<Variable>, Flow), String> {
        let prev_st = self.stack.len();
        let prev_lc = self.local_stack.len();

        let start = start!(self, for_n_expr);
        let end = end!(self, for_n_expr);

        let mut min = ::std::f64::NAN;
        let mut sec = None;
        // Initialize counter.
        self.local_stack
            .push((for_n_expr.name.clone(), self.stack.len()));
        self.stack.push(Variable::f64(start));
        let st = self.stack.len();
        let lc = self.local_stack.len();
        let mut flow = Flow::Continue;
        loop {
            let ind = cond!(self, for_n_expr, st, end);
            match self.block(&for_n_expr.block)? {
                (Some(x), Flow::Continue) => {
                    match self.resolve(&x) {
                        &Variable::F64(val, ref val_sec) => {
                            if min.is_nan() || min > val {
                                min = val;
                                sec = match *val_sec {
                                    None => Some(Box::new(vec![Variable::f64(ind)])),
                                    Some(ref arr) => {
                                        let mut arr = arr.clone();
                                        arr.push(Variable::f64(ind));
                                        Some(arr)
                                    }
                                };
                            }
                        }
                        x => {
                            return Err(self.module.error(
                                for_n_expr.block.source_range,
                                &self.expected(x, "number"),
                                self,
                            ))
                        }
                    };
                }
                (x, Flow::Return) => {
                    return Ok((x, Flow::Return));
                }
                (None, Flow::Continue) => {
                    return Err(self.module.error(
                        for_n_expr.block.source_range,
                        "Expected `number or option`",
                        self,
                    ))
                }
                (_, Flow::Break(x)) => break_!(x, for_n_expr, flow),
                (_, Flow::ContinueLoop(x)) => continue_!(x, for_n_expr, flow),
            }
            inc!(self, for_n_expr, st);
            self.stack.truncate(st);
            self.local_stack.truncate(lc);
        }
        self.stack.truncate(prev_st);
        self.local_stack.truncate(prev_lc);
        Ok((Some(Variable::F64(min, sec)), flow))
    }

    pub(crate) fn max_n_expr(
        &mut self,
        for_n_expr: &ast::ForN,
    ) -> Result<(Option<Variable>, Flow), String> {
        let prev_st = self.stack.len();
        let prev_lc = self.local_stack.len();

        let start = start!(self, for_n_expr);
        let end = end!(self, for_n_expr);

        let mut max = ::std::f64::NAN;
        let mut sec = None;
        // Initialize counter.
        self.local_stack
            .push((for_n_expr.name.clone(), self.stack.len()));
        self.stack.push(Variable::f64(start));

        let st = self.stack.len();
        let lc = self.local_stack.len();
        let mut flow = Flow::Continue;
        loop {
            let ind = cond!(self, for_n_expr, st, end);
            match self.block(&for_n_expr.block)? {
                (Some(x), Flow::Continue) => {
                    match self.resolve(&x) {
                        &Variable::F64(val, ref val_sec) => {
                            if max.is_nan() || max < val {
                                max = val;
                                sec = match *val_sec {
                                    None => Some(Box::new(vec![Variable::f64(ind)])),
                                    Some(ref arr) => {
                                        let mut arr = arr.clone();
                                        arr.push(Variable::f64(ind));
                                        Some(arr)
                                    }
                                };
                            }
                        }
                        x => {
                            return Err(self.module.error(
                                for_n_expr.block.source_range,
                                &self.expected(x, "number"),
                                self,
                            ))
                        }
                    };
                }
                (x, Flow::Return) => {
                    return Ok((x, Flow::Return));
                }
                (None, Flow::Continue) => {
                    return Err(self.module.error(
                        for_n_expr.block.source_range,
                        "Expected `number`",
                        self,
                    ))
                }
                (_, Flow::Break(x)) => break_!(x, for_n_expr, flow),
                (_, Flow::ContinueLoop(x)) => continue_!(x, for_n_expr, flow),
            }
            inc!(self, for_n_expr, st);
            self.stack.truncate(st);
            self.local_stack.truncate(lc);
        }
        self.stack.truncate(prev_st);
        self.local_stack.truncate(prev_lc);
        Ok((Some(Variable::F64(max, sec)), flow))
    }

    pub(crate) fn any_n_expr(
        &mut self,
        for_n_expr: &ast::ForN,
    ) -> Result<(Option<Variable>, Flow), String> {
        let prev_st = self.stack.len();
        let prev_lc = self.local_stack.len();

        let start = start!(self, for_n_expr);
        let end = end!(self, for_n_expr);

        let mut any = false;
        let mut sec = None;
        // Initialize counter.
        self.local_stack
            .push((for_n_expr.name.clone(), self.stack.len()));
        self.stack.push(Variable::f64(start));

        let st = self.stack.len();
        let lc = self.local_stack.len();
        let mut flow = Flow::Continue;
        loop {
            let ind = cond!(self, for_n_expr, st, end);
            match self.block(&for_n_expr.block)? {
                (Some(x), Flow::Continue) => {
                    match self.resolve(&x) {
                        &Variable::Bool(val, ref val_sec) => {
                            if val {
                                any = true;
                                sec = match *val_sec {
                                    None => Some(Box::new(vec![Variable::f64(ind)])),
                                    Some(ref arr) => {
                                        let mut arr = arr.clone();
                                        arr.push(Variable::f64(ind));
                                        Some(arr)
                                    }
                                };
                                break;
                            }
                        }
                        x => {
                            return Err(self.module.error(
                                for_n_expr.block.source_range,
                                &self.expected(x, "boolean"),
                                self,
                            ))
                        }
                    };
                }
                (x, Flow::Return) => {
                    return Ok((x, Flow::Return));
                }
                (None, Flow::Continue) => {
                    return Err(self.module.error(
                        for_n_expr.block.source_range,
                        "Expected `boolean`",
                        self,
                    ))
                }
                (_, Flow::Break(x)) => break_!(x, for_n_expr, flow),
                (_, Flow::ContinueLoop(x)) => continue_!(x, for_n_expr, flow),
            }
            inc!(self, for_n_expr, st);
            self.stack.truncate(st);
            self.local_stack.truncate(lc);
        }
        self.stack.truncate(prev_st);
        self.local_stack.truncate(prev_lc);
        Ok((Some(Variable::Bool(any, sec)), flow))
    }

    pub(crate) fn all_n_expr(
        &mut self,
        for_n_expr: &ast::ForN,
    ) -> Result<(Option<Variable>, Flow), String> {
        let prev_st = self.stack.len();
        let prev_lc = self.local_stack.len();

        let start = start!(self, for_n_expr);
        let end = end!(self, for_n_expr);

        let mut all = true;
        let mut sec = None;
        // Initialize counter.
        self.local_stack
            .push((for_n_expr.name.clone(), self.stack.len()));
        self.stack.push(Variable::f64(start));

        let st = self.stack.len();
        let lc = self.local_stack.len();
        let mut flow = Flow::Continue;
        loop {
            let ind = cond!(self, for_n_expr, st, end);
            match self.block(&for_n_expr.block)? {
                (Some(x), Flow::Continue) => {
                    match self.resolve(&x) {
                        &Variable::Bool(val, ref val_sec) => {
                            if !val {
                                all = false;
                                sec = match *val_sec {
                                    None => Some(Box::new(vec![Variable::f64(ind)])),
                                    Some(ref arr) => {
                                        let mut arr = arr.clone();
                                        arr.push(Variable::f64(ind));
                                        Some(arr)
                                    }
                                };
                                break;
                            }
                        }
                        x => {
                            return Err(self.module.error(
                                for_n_expr.block.source_range,
                                &self.expected(x, "boolean"),
                                self,
                            ))
                        }
                    };
                }
                (x, Flow::Return) => {
                    return Ok((x, Flow::Return));
                }
                (None, Flow::Continue) => {
                    return Err(self.module.error(
                        for_n_expr.block.source_range,
                        "Expected `boolean`",
                        self,
                    ))
                }
                (_, Flow::Break(x)) => break_!(x, for_n_expr, flow),
                (_, Flow::ContinueLoop(x)) => continue_!(x, for_n_expr, flow),
            }
            inc!(self, for_n_expr, st);
            self.stack.truncate(st);
            self.local_stack.truncate(lc);
        }
        self.stack.truncate(prev_st);
        self.local_stack.truncate(prev_lc);
        Ok((Some(Variable::Bool(all, sec)), flow))
    }

    pub(crate) fn link_for_n_expr(
        &mut self,
        for_n_expr: &ast::ForN,
    ) -> Result<(Option<Variable>, Flow), String> {
        use crate::Link;

        fn sub_link_for_n_expr(
            res: &mut Link,
            rt: &mut Runtime,
            for_n_expr: &ast::ForN,
        ) -> Result<(Option<Variable>, Flow), String> {
            let prev_st = rt.stack.len();
            let prev_lc = rt.local_stack.len();

            let start = start!(rt, for_n_expr);
            let end = end!(rt, for_n_expr);

            // Initialize counter.
            rt.local_stack
                .push((for_n_expr.name.clone(), rt.stack.len()));
            rt.stack.push(Variable::f64(start));

            let st = rt.stack.len();
            let lc = rt.local_stack.len();
            let mut flow = Flow::Continue;

            'outer: loop {
                cond!(rt, for_n_expr, st, end);

                match for_n_expr.block.expressions[0] {
                    ast::Expression::Link(ref link) => {
                        // Evaluate link items directly.
                        'inner: for item in &link.items {
                            match rt.expression(item, Side::Right)? {
                                (Some(ref x), Flow::Continue) => match res.push(rt.resolve(x)) {
                                    Err(err) => {
                                        return Err(rt.module.error(
                                            for_n_expr.source_range,
                                            &format!("{}\n{}", rt.stack_trace(), err),
                                            rt,
                                        ))
                                    }
                                    Ok(()) => {}
                                },
                                (x, Flow::Return) => {
                                    return Ok((x, Flow::Return));
                                }
                                (None, Flow::Continue) => {}
                                (_, Flow::Break(x)) => break_!(x, for_n_expr, flow, 'outer),
                                (_, Flow::ContinueLoop(x)) => match x {
                                    Some(label) => {
                                        let same = if let Some(ref for_label) = for_n_expr.label {
                                            &label == for_label
                                        } else {
                                            false
                                        };
                                        if !same {
                                            flow = Flow::ContinueLoop(Some(label));
                                            break 'outer;
                                        } else {
                                            break 'inner;
                                        }
                                    }
                                    None => {
                                        break 'inner;
                                    }
                                },
                            }
                        }
                    }
                    ast::Expression::LinkFor(ref for_n) => {
                        // Pass on control to next link loop.
                        match sub_link_for_n_expr(res, rt, for_n) {
                            Ok((None, Flow::Continue)) => {}
                            Ok((_, Flow::Break(x))) => break_!(x, for_n_expr, flow, 'outer),
                            Ok((_, Flow::ContinueLoop(x))) => {
                                if let Some(label) = x {
                                    let same = if let Some(ref for_label) = for_n_expr.label {
                                        &label == for_label
                                    } else {
                                        false
                                    };
                                    if !same {
                                        flow = Flow::ContinueLoop(Some(label));
                                        break 'outer;
                                    }
                                }
                            }
                            x => return x,
                        }
                    }
                    _ => {
                        panic!("Link body is not link");
                    }
                }

                inc!(rt, for_n_expr, st);
                rt.stack.truncate(st);
                rt.local_stack.truncate(lc);
            }
            rt.stack.truncate(prev_st);
            rt.local_stack.truncate(prev_lc);
            Ok((None, flow))
        }

        let mut res: Link = Link::new();
        match sub_link_for_n_expr(&mut res, self, for_n_expr) {
            Ok((None, Flow::Continue)) => Ok((Some(Variable::Link(Box::new(res))), Flow::Continue)),
            x => x,
        }
    }

    pub(crate) fn sift_n_expr(
        &mut self,
        for_n_expr: &ast::ForN,
    ) -> Result<(Option<Variable>, Flow), String> {
        let prev_st = self.stack.len();
        let prev_lc = self.local_stack.len();
        let mut res: Vec<Variable> = vec![];

        let start = start!(self, for_n_expr);
        let end = end!(self, for_n_expr);

        // Initialize counter.
        self.local_stack
            .push((for_n_expr.name.clone(), self.stack.len()));
        self.stack.push(Variable::f64(start));

        let st = self.stack.len();
        let lc = self.local_stack.len();
        let mut flow = Flow::Continue;
        loop {
            cond!(self, for_n_expr, st, end);
            match self.block(&for_n_expr.block)? {
                (Some(x), Flow::Continue) => res.push(x),
                (x, Flow::Return) => {
                    return Ok((x, Flow::Return));
                }
                (None, Flow::Continue) => {
                    return Err(self.module.error(
                        for_n_expr.block.source_range,
                        "Expected variable",
                        self,
                    ))
                }
                (_, Flow::Break(x)) => break_!(x, for_n_expr, flow),
                (_, Flow::ContinueLoop(x)) => continue_!(x, for_n_expr, flow),
            }
            inc!(self, for_n_expr, st);
            self.stack.truncate(st);
            self.local_stack.truncate(lc);
        }
        self.stack.truncate(prev_st);
        self.local_stack.truncate(prev_lc);
        Ok((Some(Variable::Array(Arc::new(res))), flow))
    }

    pub(crate) fn sum_vec4_n_expr(
        &mut self,
        for_n_expr: &ast::ForN,
    ) -> Result<(Option<Variable>, Flow), String> {
        let prev_st = self.stack.len();
        let prev_lc = self.local_stack.len();
        let mut sum: [f32; 4] = [0.0; 4];

        let start = start!(self, for_n_expr);
        let end = end!(self, for_n_expr);

        // Initialize counter.
        self.local_stack
            .push((for_n_expr.name.clone(), self.stack.len()));
        self.stack.push(Variable::f64(start));

        let st = self.stack.len();
        let lc = self.local_stack.len();
        let mut flow = Flow::Continue;
        loop {
            cond!(self, for_n_expr, st, end);
            match self.block(&for_n_expr.block)? {
                (Some(x), Flow::Continue) => {
                    match self.resolve(&x) {
                        &Variable::Vec4(val) => {
                            for i in 0..4 {
                                sum[i] += val[i]
                            }
                        }
                        x => {
                            return Err(self.module.error(
                                for_n_expr.block.source_range,
                                &self.expected(x, "vec4"),
                                self,
                            ))
                        }
                    };
                }
                (x, Flow::Return) => {
                    return Ok((x, Flow::Return));
                }
                (None, Flow::Continue) => {
                    return Err(self.module.error(
                        for_n_expr.block.source_range,
                        "Expected `vec4`",
                        self,
                    ))
                }
                (_, Flow::Break(x)) => break_!(x, for_n_expr, flow),
                (_, Flow::ContinueLoop(x)) => continue_!(x, for_n_expr, flow),
            }
            inc!(self, for_n_expr, st);
            self.stack.truncate(st);
            self.local_stack.truncate(lc);
        }
        self.stack.truncate(prev_st);
        self.local_stack.truncate(prev_lc);
        Ok((Some(Variable::Vec4(sum)), flow))
    }

    pub(crate) fn prod_vec4_n_expr(
        &mut self,
        for_n_expr: &ast::ForN,
    ) -> Result<(Option<Variable>, Flow), String> {
        let prev_st = self.stack.len();
        let prev_lc = self.local_stack.len();
        let mut prod: [f32; 4] = [1.0; 4];

        let start = start!(self, for_n_expr);
        let end = end!(self, for_n_expr);

        // Initialize counter.
        self.local_stack
            .push((for_n_expr.name.clone(), self.stack.len()));
        self.stack.push(Variable::f64(start));

        let st = self.stack.len();
        let lc = self.local_stack.len();
        let mut flow = Flow::Continue;
        loop {
            cond!(self, for_n_expr, st, end);
            match self.block(&for_n_expr.block)? {
                (Some(x), Flow::Continue) => {
                    match self.resolve(&x) {
                        &Variable::Vec4(val) => {
                            for i in 0..4 {
                                prod[i] *= val[i]
                            }
                        }
                        x => {
                            return Err(self.module.error(
                                for_n_expr.block.source_range,
                                &self.expected(x, "vec4"),
                                self,
                            ))
                        }
                    };
                }
                (x, Flow::Return) => {
                    return Ok((x, Flow::Return));
                }
                (None, Flow::Continue) => {
                    return Err(self.module.error(
                        for_n_expr.block.source_range,
                        "Expected `vec4`",
                        self,
                    ))
                }
                (_, Flow::Break(x)) => break_!(x, for_n_expr, flow),
                (_, Flow::ContinueLoop(x)) => continue_!(x, for_n_expr, flow),
            }
            inc!(self, for_n_expr, st);
            self.stack.truncate(st);
            self.local_stack.truncate(lc);
        }
        self.stack.truncate(prev_st);
        self.local_stack.truncate(prev_lc);
        Ok((Some(Variable::Vec4(prod)), flow))
    }
}
