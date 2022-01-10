use std::sync::Arc;

use piston_meta::bootstrap::Convert;
use range::Range;
use Dfn;

/// Stores a Dyon type.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// Whether a statement is never reached.
    Unreachable,
    /// A no-type.
    Void,
    /// Any type.
    Any,
    /// Boolean type.
    Bool,
    /// F64 type.
    F64,
    /// 4D vector type.
    Vec4,
    /// 4D matrix type.
    Mat4,
    /// String/text type.
    Str,
    /// Link type.
    Link,
    /// Array type.
    Array(Box<Type>),
    /// Object type.
    Object,
    /// Option type.
    Option(Box<Type>),
    /// Result type.
    Result(Box<Type>),
    /// Secret type.
    Secret(Box<Type>),
    /// Thread handle type.
    #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
    Thread(Box<Type>),
    /// In-type.
    In(Box<Type>),
    /// Ad-hoc type.
    AdHoc(Arc<String>, Box<Type>),
    /// Closure type.
    Closure(Box<Dfn>),
}

impl Type {
    /// Returns an extension quantified over ad-hoc types.
    ///
    /// For example, `(vec4, vec4) -> vec4` becomes `all T { (T vec4, T vec4) -> T vec4 }`.
    pub fn all_ext(args: Vec<Type>, ret: Type) -> (Vec<Arc<String>>, Vec<Type>, Type) {
        use crate::T;
        use Type::AdHoc;

        (
            vec![T.clone()],
            args.into_iter()
                .map(|arg| AdHoc(T.clone(), Box::new(arg)))
                .collect(),
            AdHoc(T.clone(), Box::new(ret)),
        )
    }

    /// Returns description of the type.
    pub fn description(&self) -> String {
        use Type::*;

        match *self {
            Unreachable => "unreachable".into(),
            Void => "void".into(),
            Any => "any".into(),
            Bool => "bool".into(),
            F64 => "f64".into(),
            Vec4 => "vec4".into(),
            Mat4 => "mat4".into(),
            Str => "str".into(),
            Link => "link".into(),
            Array(ref ty) => {
                if let Any = **ty {
                    "[]".into()
                } else {
                    let mut res = String::from("[");
                    res.push_str(&ty.description());
                    res.push(']');
                    res
                }
            }
            Object => "{}".into(),
            Option(ref ty) => {
                if let Any = **ty {
                    "opt".into()
                } else {
                    let mut res = String::from("opt[");
                    res.push_str(&ty.description());
                    res.push(']');
                    res
                }
            }
            Result(ref ty) => {
                if let Any = **ty {
                    "res".into()
                } else {
                    let mut res = String::from("res[");
                    res.push_str(&ty.description());
                    res.push(']');
                    res
                }
            }
            Secret(ref ty) => match **ty {
                Bool => "sec[bool]".into(),
                F64 => "sec[f64]".into(),
                _ => panic!("Secret only supports `bool` and `f64`"),
            },
            #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
            Thread(ref ty) => {
                if let Any = **ty {
                    "thr".into()
                } else {
                    let mut res = String::from("thr[");
                    res.push_str(&ty.description());
                    res.push(']');
                    res
                }
            }
            In(ref ty) => {
                if let Any = **ty {
                    "in".into()
                } else {
                    let mut res = String::from("in[");
                    res.push_str(&ty.description());
                    res.push(']');
                    res
                }
            }
            AdHoc(ref ad, ref ty) => (&**ad).clone() + " " + &ty.description(),
            Closure(ref closure) => {
                let mut s = String::new();
                s.push_str("\\(");
                for (i, ty) in closure.tys.iter().enumerate() {
                    s.push_str(&ty.description());
                    if i + 1 < closure.tys.len() {
                        s.push_str(", ");
                    }
                }
                s.push_str(") -> ");
                s.push_str(&closure.ret.description());
                s
            }
        }
    }

    /// Returns an array type with an `any` as inner type.
    pub fn array() -> Type {
        Type::Array(Box::new(Type::Any))
    }

    /// Returns an object type.
    pub fn object() -> Type {
        Type::Object
    }

    /// Returns an Option type with an `any` as inner type.
    pub fn option() -> Type {
        Type::Option(Box::new(Type::Any))
    }

    /// Returns a Result type with an `any` as inner type.
    pub fn result() -> Type {
        Type::Result(Box::new(Type::Any))
    }

    /// Returns a thread handle type with an `any` as inner type.
    #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
    pub fn thread() -> Type {
        Type::Thread(Box::new(Type::Any))
    }

    /// Returns an in-type with an `any` as inner type.
    pub fn in_ty() -> Type {
        Type::In(Box::new(Type::Any))
    }

    /// Binds refinement type variables.
    ///
    /// Returns the type argument to compare to.
    pub fn bind_ty_vars(
        &self,
        refine: &Type,
        names: &[Arc<String>],
        ty_vars: &mut Vec<Option<Arc<String>>>,
    ) -> Result<Type, String> {
        if names.is_empty() {
            return Ok(self.clone());
        };
        match (self, refine) {
            (
                &Type::AdHoc(ref a_name, ref a_inner_ty),
                &Type::AdHoc(ref b_name, ref b_inner_ty),
            ) => {
                for i in 0..names.len() {
                    if a_name == &names[i] {
                        let new_inner = a_inner_ty.bind_ty_vars(b_inner_ty, names, ty_vars)?;
                        if let Some(ref existing_name) = ty_vars[i] {
                            if existing_name != b_name
                                && new_inner.goes_with(b_inner_ty)
                                && !new_inner.ambiguous(b_inner_ty)
                            {
                                return Err(format!(
                                    "Type mismatch (#1500): Expected `{}`, found `{}`",
                                    existing_name, b_name
                                ));
                            } else {
                                return Ok(Type::AdHoc(existing_name.clone(), Box::new(new_inner)));
                            }
                        } else {
                            ty_vars[i] = Some(b_name.clone());
                            return Ok(Type::AdHoc(
                                b_name.clone(),
                                Box::new(a_inner_ty.bind_ty_vars(b_inner_ty, names, ty_vars)?),
                            ));
                        }
                    }
                }
                Ok(Type::AdHoc(
                    a_name.clone(),
                    Box::new(a_inner_ty.bind_ty_vars(b_inner_ty, names, ty_vars)?),
                ))
            }
            (&Type::AdHoc(ref a_name, ref a_inner_ty), b) => {
                for i in 0..names.len() {
                    if a_name == &names[i] {
                        let new_inner = a_inner_ty.bind_ty_vars(refine, names, ty_vars)?;
                        if let Some(ref n) = ty_vars[i] {
                            if new_inner.goes_with(b) && !new_inner.ambiguous(b) {
                                return Err(format!(
                                    "Type mismatch (#1600): Expected `{}`, found no ad-hoc type",
                                    n
                                ));
                            }
                        } else {
                            break;
                        }
                    }
                }
                a_inner_ty.bind_ty_vars(refine, names, ty_vars)
            }
            _ => Ok(self.clone()),
        }
    }

    /// Inserts variable name, replacing ad-hoc type name.
    pub fn insert_var(&mut self, name: &Arc<String>, val: &Arc<String>) {
        match *self {
            Type::AdHoc(ref mut n, ref mut inner_ty) => {
                if n == name {
                    *n = val.clone();
                }
                inner_ty.insert_var(name, val)
            }
            _ => {}
        }
    }

    /// Inserts a none ad-hoc variable.
    pub fn insert_none_var(&mut self, name: &Arc<String>) {
        match *self {
            Type::AdHoc(_, ref mut inner_ty) => {
                inner_ty.insert_none_var(name);
                *self = (**inner_ty).clone();
            }
            _ => {}
        }
    }

    /// Returns `true` if a type to be refined is ambiguous relative to this type (directional check).
    ///
    /// For example, the type ad-hoc type `Foo str` is ambiguous with type `str`.
    /// If more was known about the `str` type with further refinement,
    /// then it might turn out to be `Bar str`, which triggers a collision.
    pub fn ambiguous(&self, refine: &Type) -> bool {
        use self::Type::*;

        match (self, refine) {
            (&AdHoc(ref xa, ref xb), &AdHoc(ref ya, ref yb)) if xa == ya => xb.ambiguous(yb),
            (&AdHoc(_, ref x), y) if x.goes_with(y) => true,
            (&Array(ref x), &Array(ref y)) if x.ambiguous(y) => true,
            (&Option(ref x), &Option(ref y)) if x.ambiguous(y) => true,
            (&Result(ref x), &Result(ref y)) if x.ambiguous(y) => true,
            #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
            (&Thread(ref x), &Thread(ref y)) if x.ambiguous(y) => true,
            (&In(ref x), &In(ref y)) if x.ambiguous(y) => true,
            (&Bool, &Any) => true,
            (&F64, &Any) => true,
            (&Str, &Any) => true,
            (&Vec4, &Any) => true,
            (&Mat4, &Any) => true,
            (&Link, &Any) => true,
            (&Array(_), &Any) => true,
            (&Option(_), &Any) => true,
            (&Result(_), &Any) => true,
            #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
            (&Thread(_), &Any) => true,
            (&Secret(_), &Any) => true,
            (&In(_), &Any) => true,
            _ => false,
        }
    }

    /// Returns `true` if the type can be a closure, `false` otherwise.
    pub fn closure_ret_ty(&self) -> Option<Type> {
        use self::Type::*;

        match *self {
            Closure(ref ty) => Some(ty.ret.clone()),
            AdHoc(_, ref x) => x.closure_ret_ty(),
            Any => Some(Type::Any),
            _ => None,
        }
    }

    /// Returns `true` if a type goes with another type (directional check).
    ///
    /// - `bool` (argument) goes with `sec[bool]` (value)
    /// - `f64` (argument) goes with `sec[f64]` (value)
    ///
    /// The opposite is not true, since `sec` contains extra information.
    pub fn goes_with(&self, other: &Type) -> bool {
        use self::Type::*;

        // Invert the order because of complex ad-hoc logic.
        if let AdHoc(_, _) = *other {
            if let AdHoc(_, _) = *self {
            } else {
                return other.goes_with(self);
            }
        }
        if let Secret(ref other_ty) = *other {
            return if let Secret(ref this_ty) = *self {
                this_ty.goes_with(other_ty)
            } else {
                self.goes_with(other_ty)
            };
        }
        match self {
            // Unreachable goes with anything.
            &Unreachable => true,
            _ if *other == Unreachable => true,
            &Any => *other != Void,
            // Void only goes with void.
            &Void => *other == Void,
            &Array(ref arr) => {
                if let Array(ref other_arr) = *other {
                    arr.goes_with(other_arr)
                } else {
                    matches!(*other, Any)
                }
            }
            &Object => {
                if let Object = *other {
                    true
                } else {
                    matches!(*other, Any)
                }
            }
            &Option(ref opt) => {
                if let Option(ref other_opt) = *other {
                    opt.goes_with(other_opt)
                } else {
                    matches!(*other, Any)
                }
            }
            &Result(ref res) => {
                if let Result(ref other_res) = *other {
                    res.goes_with(other_res)
                } else if let Any = *other {
                    true
                } else {
                    false
                }
            }
            #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
            &Thread(ref thr) => {
                if let Thread(ref other_thr) = *other {
                    thr.goes_with(other_thr)
                } else if let Any = *other {
                    true
                } else {
                    false
                }
            }
            &In(ref in_ty) => {
                if let In(ref other_ty) = *other {
                    in_ty.goes_with(other_ty)
                } else if let Any = *other {
                    true
                } else {
                    false
                }
            }
            &Closure(ref cl) => {
                if let Closure(ref other_cl) = *other {
                    if cl.tys.len() != other_cl.tys.len() {
                        return false;
                    }
                    if !cl
                        .tys
                        .iter()
                        .zip(other_cl.tys.iter())
                        .all(|(a, b)| a.goes_with(b))
                    {
                        return false;
                    }
                    if !cl.ret.goes_with(&other_cl.ret) {
                        return false;
                    }
                    true
                } else if let Any = *other {
                    true
                } else {
                    false
                }
            }
            &AdHoc(ref name, ref ty) => {
                if let AdHoc(ref other_name, ref other_ty) = *other {
                    name == other_name && ty.goes_with(other_ty)
                } else if let Void = *other {
                    false
                } else {
                    ty.goes_with(other)
                }
            }
            // Bool, F64, Text, Vec4.
            x if x == other => true,
            _ if *other == Type::Any => true,
            _ => false,
        }
    }

    /// Infers type from the `+=` operator.
    pub fn add_assign(&self, other: &Type) -> bool {
        use self::Type::*;

        match (self, other) {
            (&AdHoc(ref name, ref ty), &AdHoc(ref other_name, ref other_ty)) => {
                if name != other_name {
                    return false;
                }
                if !ty.goes_with(other_ty) {
                    return false;
                }
                ty.add_assign(other_ty)
            }
            (&AdHoc(_, _), _) | (_, &AdHoc(_, _)) => false,
            (&Void, _) | (_, &Void) => false,
            _ => true,
        }
    }

    /// Converts meta data into a type.
    pub fn from_meta_data(
        node: &str,
        mut convert: Convert,
        ignored: &mut Vec<Range>,
    ) -> Result<(Range, Type), ()> {
        let start = convert;
        let start_range = convert.start_node(node)?;
        convert.update(start_range);

        let mut ty: Option<Type> = None;
        loop {
            if let Ok(range) = convert.end_node(node) {
                convert.update(range);
                break;
            } else if let Ok((range, _)) = convert.meta_bool("any") {
                convert.update(range);
                ty = Some(Type::Any);
            } else if let Ok((range, _)) = convert.meta_bool("bool") {
                convert.update(range);
                ty = Some(Type::Bool);
            } else if let Ok((range, _)) = convert.meta_bool("sec_bool") {
                convert.update(range);
                ty = Some(Type::Secret(Box::new(Type::Bool)));
            } else if let Ok((range, _)) = convert.meta_bool("f64") {
                convert.update(range);
                ty = Some(Type::F64);
            } else if let Ok((range, _)) = convert.meta_bool("sec_f64") {
                convert.update(range);
                ty = Some(Type::Secret(Box::new(Type::F64)));
            } else if let Ok((range, _)) = convert.meta_bool("str") {
                convert.update(range);
                ty = Some(Type::Str);
            } else if let Ok((range, _)) = convert.meta_bool("vec4") {
                convert.update(range);
                ty = Some(Type::Vec4);
            } else if let Ok((range, _)) = convert.meta_bool("mat4") {
                convert.update(range);
                ty = Some(Type::Mat4);
            } else if let Ok((range, _)) = convert.meta_bool("link") {
                convert.update(range);
                ty = Some(Type::Link);
            } else if let Ok((range, _)) = convert.meta_bool("opt_any") {
                convert.update(range);
                ty = Some(Type::Option(Box::new(Type::Any)));
            } else if let Ok((range, _)) = convert.meta_bool("res_any") {
                convert.update(range);
                ty = Some(Type::Result(Box::new(Type::Any)));
            } else if let Ok((range, _)) = convert.meta_bool("arr_any") {
                convert.update(range);
                ty = Some(Type::Array(Box::new(Type::Any)));
            } else if let Ok((range, _)) = convert.meta_bool("obj_any") {
                convert.update(range);
                ty = Some(Type::Object);
            } else if let Ok((range, _)) = convert.meta_bool("in_any") {
                convert.update(range);
                ty = Some(Type::In(Box::new(Type::Any)));
            } else if let Ok((range, val)) = Type::from_meta_data("opt", convert, ignored) {
                convert.update(range);
                ty = Some(Type::Option(Box::new(val)));
            } else if let Ok((range, val)) = Type::from_meta_data("res", convert, ignored) {
                convert.update(range);
                ty = Some(Type::Result(Box::new(val)));
            } else if let Ok((range, val)) = Type::from_meta_data("arr", convert, ignored) {
                convert.update(range);
                ty = Some(Type::Array(Box::new(val)));
            } else if let Ok((range, val)) = Type::from_meta_data("in", convert, ignored) {
                convert.update(range);
                ty = Some(Type::In(Box::new(val)));
            } else if let Ok((range, val)) = convert.meta_string("ad_hoc") {
                convert.update(range);
                let inner_ty =
                    if let Ok((range, val)) = Type::from_meta_data("ad_hoc_ty", convert, ignored) {
                        convert.update(range);
                        val
                    } else {
                        Type::Object
                    };
                ty = Some(Type::AdHoc(val, Box::new(inner_ty)));
            } else if let Ok(range) = convert.start_node("closure_type") {
                convert.update(range);
                let mut lts = vec![];
                let mut tys = vec![];
                while let Ok((range, val)) = Type::from_meta_data("cl_arg", convert, ignored) {
                    use Lt;

                    convert.update(range);
                    lts.push(Lt::Default);
                    tys.push(val);
                }
                let (range, ret) = Type::from_meta_data("cl_ret", convert, ignored)?;
                convert.update(range);
                let range = convert.end_node("closure_type")?;
                convert.update(range);
                ty = Some(Type::Closure(Box::new(Dfn {
                    lts,
                    tys,
                    ret,
                    ext: vec![],
                    lazy: crate::LAZY_NO,
                })));
            } else {
                loop {
                    #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
                    if let Ok((range, _)) = convert.meta_bool("thr_any") {
                        convert.update(range);
                        ty = Some(Type::Thread(Box::new(Type::Any)));
                        break;
                    }
                    #[cfg(all(not(target_family = "wasm"), feature = "threading"))]
                    if let Ok((range, val)) = Type::from_meta_data("thr", convert, ignored) {
                        convert.update(range);
                        ty = Some(Type::Thread(Box::new(val)));
                        break;
                    }
                    let range = convert.ignore();
                    convert.update(range);
                    ignored.push(range);
                    break;
                }
            }
        }

        Ok((convert.subtract(start), ty.ok_or(())?))
    }
}
