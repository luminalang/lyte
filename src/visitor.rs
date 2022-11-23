use super::{Type, TypeData, TypeKind, TypesBuf};

pub trait TypeVisitor<D: TypeData> {
    fn map_types<F: FnMut(D::Meta, &TypeKind<D>, TypesBuf<D>) -> Type<D>>(&self, f: F) -> Self
    where
        Self: Sized;

    fn map_constr<F: FnMut(D::Meta, &TypeKind<D>) -> TypeKind<D>>(&self, mut f: F) -> Self
    where
        Self: Sized,
    {
        self.map_types(|meta, constr, params| Type {
            constr: f(meta.clone(), constr),
            meta,
            params,
        })
    }
}

impl<D: TypeData> Type<D> {
    pub fn map_constr<
        O: TypeData<Meta = D::Meta>,
        F: FnMut(D::Meta, &TypeKind<D>) -> TypeKind<O>,
    >(
        &self,
        f: &mut F,
    ) -> Type<O> {
        self.map_type(&mut |meta, constr, params| Type {
            constr: f(meta.clone(), constr),
            meta,
            params,
        })
    }

    pub fn map_type<O: TypeData, F: FnMut(D::Meta, &TypeKind<D>, Vec<Type<O>>) -> Type<O>>(
        &self,
        f: &mut F,
    ) -> Type<O> {
        let params = self.params.iter().map(|t| t.map_type(f)).collect();
        f(self.meta.clone(), &self.constr, params)
    }

    pub fn into_map_constr<O: TypeData<Meta = D::Meta>, F: FnMut(TypeKind<D>) -> TypeKind<O>>(
        self,
        f: &mut F,
    ) -> Type<O> {
        self.into_map_type(&mut |meta, constr, params| Type {
            meta,
            constr: f(constr),
            params,
        })
    }

    pub fn into_map_type<
        O: TypeData<Meta = D::Meta>,
        F: FnMut(D::Meta, TypeKind<D>, Vec<Type<O>>) -> Type<O>,
    >(
        self,
        f: &mut F,
    ) -> Type<O> {
        let params = self
            .params
            .into_iter()
            .map(|t| t.into_map_type(f))
            .collect();
        f(self.meta, self.constr, params)
    }

    pub fn substitute_self_and_meta(&self, impltor: &Type<D>, or_meta: D::Meta) -> Type<D> {
        self.map_type(&mut |_, constr, params| match constr {
            TypeKind::Self_ => {
                let mut type_ = impltor.clone();
                type_.params.extend(params);
                type_
            }
            _ => Type {
                constr: constr.clone(),
                params,
                meta: or_meta.clone(),
            },
        })
    }
}
