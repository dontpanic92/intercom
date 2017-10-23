#![crate_type="dylib"]
#![feature(quote, plugin_registrar, rustc_private)]

extern crate syntax;
extern crate syntax_pos;
extern crate rustc;
extern crate rustc_plugin;
extern crate com_runtime;

use syntax::ptr::P;
use syntax::symbol::Symbol;
use syntax::codemap::Span;
use syntax::ext::base::{ExtCtxt, Annotatable};
use syntax::ext::base::SyntaxExtension::{MultiDecorator};
use syntax::tokenstream::TokenTree;
use syntax::ast::{
    Ident,
    Item, ItemKind, ImplItemKind,
    MetaItem, MetaItemKind, NestedMetaItemKind, LitKind,
    MutTy, Ty, TyKind, FunctionRetTy,
    PathParameters, Mutability,
    Attribute,
};
use rustc_plugin::Registry;
use syntax::print::pprust;

pub fn repr_c( cx : &mut ExtCtxt ) -> Attribute
{
    quote_attr!( cx, #[repr(C)] )
}

pub fn get_ret_types(
    cx: &mut ExtCtxt,
    ret_ty : &Ty
) -> Result< ( Option<P<Ty>>, P<Ty> ), &'static str >
{
    // Get the path type on the return value.
    let path = match &ret_ty.node {
        &TyKind::Path( _, ref p ) => p,
        _ => return Err( "Use Result as a return type" )
    };

    // Find the last path segment.
    let last_segment = path.segments.last().unwrap();

    // Check the last segment has angle bracketed parameters.
    if let &Some( ref p ) = &last_segment.parameters {
        if let PathParameters::AngleBracketed( ref data ) = **p {

            // Angle bracketed parameters exist. We're assuming this is
            // some kind of Result<ok> or Result<ok, err>. In either case
            // we can take the first parameter as the 'ok' type.
            //
            // TODO: Figure out whether we can ask the compiler whether
            // the type matches Result<S,E> type.
            return Ok( (
                data.types.first().and_then( |x| Some( x.clone() ) ),
                quote_ty!( cx, u32 )
            ) )
        }
    }

    // Default value. We get here only if we didn't return a type from
    // the if statements above.
    Ok( ( None, quote_ty!( cx, $path ) ) )
}

pub fn get_com_ptr_ident(
    _cx: &mut ExtCtxt,
    struct_name: &Ident
) -> Ident
{
    Ident::from_str( &format!( "__{}_ptr", struct_name ) )
}

pub fn get_clsid_ident(
    _cx: &mut ExtCtxt,
    struct_name: &Ident
) -> Ident
{
    Ident::from_str( &format!( "CLSID_{}", struct_name ) )
}

pub fn get_method_impl_ident(
    _cx: &mut ExtCtxt,
    struct_name : &str,
    itf_name : &str,
    method_name: &str
) -> Ident
{
    Ident::from_str( &format!( "__{}_{}_{}",
            struct_name, itf_name, method_name ) )
}

pub fn get_guid_tokens(
    cx : &mut ExtCtxt,
    g : &com_runtime::GUID
) -> Vec<TokenTree>
{
    let d1 = g.data1;
    let d2 = g.data2;
    let d3 = g.data3;
    let d4_0 = g.data4[ 0 ];
    let d4_1 = g.data4[ 1 ];
    let d4_2 = g.data4[ 2 ];
    let d4_3 = g.data4[ 3 ];
    let d4_4 = g.data4[ 4 ];
    let d4_5 = g.data4[ 5 ];
    let d4_6 = g.data4[ 6 ];
    let d4_7 = g.data4[ 7 ];
    quote_tokens!( cx,
        com_runtime::GUID {
            data1: $d1, data2: $d2, data3: $d3,
            data4: [ $d4_0, $d4_1, $d4_2, $d4_3, $d4_4, $d4_5, $d4_6, $d4_7 ]
        }
    )
}

pub fn impl_add_ref_release(
    cx: &mut ExtCtxt,
    com_ptr_ident : Ident,
    add_ref_impl_ident : Ident,
    release_impl_ident : Ident,
) -> ( P<Item>, P<Item> )
{
    (
        quote_item!( cx,
            #[allow(non_snake_case)]
            #[allow(dead_code)]
            pub unsafe extern "stdcall" fn $add_ref_impl_ident(
                self_void : com_runtime::ComPtr
            ) -> u32 {
                let self_ptr : *mut $com_ptr_ident
                        = std::mem::transmute( self_void );
                (*self_ptr).rc += 1;
                (*self_ptr).rc
            } ).unwrap(),
        quote_item!( cx,
            #[allow(non_snake_case)]
            #[allow(dead_code)]
            pub unsafe extern "stdcall" fn $release_impl_ident(
                self_void : com_runtime::ComPtr
            ) -> u32 {
                let self_ptr : *mut $com_ptr_ident
                        = std::mem::transmute( self_void );

                // We need a copy of the rc value in case we end up
                // dropping the ptr. We can't reference it during
                // return at that point.
                (*self_ptr).rc -= 1;
                let rc = (*self_ptr).rc;
                if rc == 0 {
                    // Take ownership of the ptr and let it go out
                    // of scope to destroy it.
                    Box::from_raw( self_ptr );
                }
                rc
            } ).unwrap()
    )
}

pub fn try_expand_com_visible(
    cx: &mut ExtCtxt,
    _sp: Span,
    mi: &MetaItem,
    item: &Annotatable,
    push: &mut FnMut( Annotatable )
) -> Result< (), &'static str >
{
    // Get the annotated item.
    let impl_item = match item {
        &Annotatable::Item( ref itf ) => itf,
        _ => return Err( "[com_visible(clsid : &str)] must be applied to impl" )
    };

    // Get the trait information from the item.
    let ( struct_ty, impl_items ) = match impl_item.node {
        ItemKind::Impl( _, _, _, _, _, ref ty, ref items ) => ( ty, items ),
        _ => return Err( "[com_visible(clsid : &str)] must be applied to impl" )
    };

    // Get the trait information from the item.
    let attr_params = match mi.node {
        MetaItemKind::List( ref v ) => v,
        _ => return Err( "[com_visible(clsid : &str) must have CLSID as a parameter." )
    };

    let clsid_item = attr_params.first()
            .ok_or( "[com_visible(clsid : &str) must have CLSID as a parameter." )?;
    let clsid_lit = match clsid_item.node {
        NestedMetaItemKind::Literal( ref l ) => &l.node,
        _ => return Err( "[com_visible(clsid : &str) must have CLSID as a parameter." )
    };
    let clsid_sym = match clsid_lit {
        &LitKind::Str( s, _ ) => s,
        _ => return Err( "[com_visible(clsid : &str) must have CLSID as a parameter." )
    };

    let clsid_guid = com_runtime::GUID::parse( &clsid_sym.as_str() )?;

    let struct_name = match struct_ty.node {
        TyKind::Path( _, ref p ) =>
                p.segments.last().unwrap().identifier.clone(),
        _ => return Err( "Could not find the interface name" )
    };
    let itf_name = format!( "I{}", struct_name );
    let com_ptr_ident = get_com_ptr_ident( cx, &struct_name );

    // Create the base vtable field. This references IUnknown.
    let mut fields = vec![
        quote_tokens!( cx, __base : com_runtime::__IUnknown_vtable, )
    ];

    // Create the vtable instance fields for IUnknown.
    let query_interface_impl_ident = get_method_impl_ident(
            cx, &struct_name.name.as_str(), &itf_name, "query_interface" );
    let add_ref_impl_ident = get_method_impl_ident(
            cx, &struct_name.name.as_str(), &itf_name, "add_ref" );
    let release_impl_ident = get_method_impl_ident(
            cx, &struct_name.name.as_str(), &itf_name, "release" );
    let mut field_values = vec![
        quote_tokens!( cx, __base : com_runtime::__IUnknown_vtable {
                query_interface : $query_interface_impl_ident,
                add_ref : $add_ref_impl_ident,
                release : $release_impl_ident
            }, )
    ];

    // IUnknown implementation.
    push( Annotatable::Item( quote_item!( cx,
            #[allow(non_snake_case)]
            #[allow(dead_code)]
            pub unsafe extern "stdcall" fn $query_interface_impl_ident(
                self_void : com_runtime::ComPtr,
                riid : com_runtime::REFIID,
                out : *mut com_runtime::ComPtr
            ) -> u32 {

                // For now only accept our own GUIDs (starting with 12341234)
                // and IUnknown (we'll use start of 00000000 to recognize this).
                //
                // Proper implementation would need
                // - proper IIDs for our interfaces. Currently we have only
                //   CLSIDs for our types.
                // - Some kind of lookup table per type to figureo out which
                //   interfaces it implements.
                if (*riid).data1 != 0x12341234 &&
                    (*riid).data1 != 0x00000000 {
                    println!( "Nope!" );
                    return com_runtime::E_NOINTERFACE
                }

                // Query interface needs to increment RC.
                let self_ptr : *mut $com_ptr_ident
                        = std::mem::transmute( self_void );
                (*self_ptr).rc += 1;
                *out = self_void;
                com_runtime::S_OK
            } ).unwrap() ) );

    let ( add_ref_impl, release_impl ) = impl_add_ref_release(
            cx, com_ptr_ident, add_ref_impl_ident, release_impl_ident );
    push( Annotatable::Item( add_ref_impl ) );
    push( Annotatable::Item( release_impl ) );

    // Process the impl items.
    for impl_item in impl_items {

        // Ensure we're processing a method item.
        let method_sig = match impl_item.node {
            ImplItemKind::Method( ref method_sig, _ ) => method_sig,
            _ => continue
        };

        let ( self_arg, other_args ) = match method_sig.decl.inputs.split_first() {
            Some( split ) => split,
            _ => continue
        };

        // Resolve the self struct and pointer types.
        let ( _ty, self_ptr_ty, self_void_ty ) = match self_arg.ty.node {
            TyKind::Rptr( _, MutTy { ref ty, mutbl } ) =>
                if mutbl == Mutability::Mutable {
                    ( ty,
                        quote_ty!( cx, *mut $com_ptr_ident ),
                        quote_ty!( cx, com_runtime::ComPtr ) )
                } else {
                    ( ty,
                        quote_ty!( cx, *const $com_ptr_ident ),
                        quote_ty!( cx, com_runtime::ComPtr ) )
                },
            _ => { continue }
        };

        // Define the arg and params array.
        // Args starts with the self arg. This is implicit for params.
        let mut args = vec![ quote_tokens!( cx, self_void : $self_void_ty, ) ];
        let mut params : Vec<Vec<TokenTree>> = vec![];
        
        // Process the remaining args into the args and params arrays.
        for ref arg_ref in other_args {
            
            // Get the arg for the args.
            let arg = arg_ref.clone();
            args.push( quote_tokens!( cx, $arg, ) );

            // We can't just clone the arg name into param name. This will cause
            // errors. I suspect this is because Rust attempts to use the same
            // tokens for two different purposes that are represented by
            // different AST nodes.
            let param_name = Ident::from_str(
                    &pprust::pat_to_string( &(*arg_ref.pat) ) );
            params.push( quote_tokens!( cx, $param_name, ) );
        }


        let output = &method_sig.decl.output;
        let ret_ty = match output {
            &FunctionRetTy::Ty( ref ty ) => ty,
            _ => continue
        };
        let ( out_val, ret_val ) = get_ret_types( cx, &ret_ty )?;

        let rval_handling = match &out_val {
            &Some( ref t ) => {
                args.push(
                        quote_tokens!( cx, __out_val : *mut $t ) );
                quote_tokens!( cx,
                    match result {
                        Ok( r ) => { *__out_val = r; return 0; },
                        Err( e ) => { return e; }
                    }; )
            },
            &None => quote_tokens!( cx, return result; )
        };

        let method_name = impl_item.ident;
        let vtable_method_name = get_method_impl_ident(
            cx,
            &struct_name.name.as_str(),
            &itf_name,
            &method_name.name.as_str() );
        let vtable_method_name_str
                = format!( "Method: {}", vtable_method_name.name );
        let method_impl = quote_item!(
            cx,
            #[allow(non_snake_case)]
            #[allow(dead_code)]
            pub unsafe extern "stdcall" fn $vtable_method_name( $args ) -> $ret_val {

                let self_struct : $self_ptr_ty = std::mem::transmute( self_void );
                let result = (*self_struct).$method_name( $params );
                $rval_handling
            }
        ).unwrap();
        push( Annotatable::Item( method_impl ) );

        // Create the struct field and add it to the fields vector.
        let vtable_method_ident = impl_item.ident;
        let vtable_method_decl = quote_tokens!(
                cx,
                #[allow(dead_code)]
                $vtable_method_ident : unsafe extern "stdcall" fn( $args ) -> $ret_val,
        );
        fields.push( vtable_method_decl );

        let vtable_method_impl = quote_tokens!(
                cx,
                $vtable_method_ident : $vtable_method_name,
        );
        field_values.push( vtable_method_impl );
    }

    // Create the vtable.
    let vtable_ident = Ident::from_str(
            format!( "__{}_vtable", itf_name ).as_str() );
    let vtable = quote_item!(
        cx,
        #[allow(non_camel_case_types)]
        struct $vtable_ident { $fields }
    ).unwrap();
    push( Annotatable::Item( vtable ) );

    let vtable_instance_name = Ident::from_str(
            &format!( "__{}_{}_vtable_instance",
                     struct_name, itf_name ) );
    let vtable_instance = quote_item!(
        cx,
        #[allow(non_upper_case_globals)]
        const $vtable_instance_name : $vtable_ident =
                $vtable_ident {
                    $field_values
                };
    ).unwrap();
    push( Annotatable::Item( vtable_instance ) );

    let clsid_ident = get_clsid_ident( cx, &struct_name );
    let clsid_guid_tokens = get_guid_tokens( cx, &clsid_guid );
    let clsid_const = quote_item!(
        cx,
        #[allow(non_upper_case_globals)]
        const $clsid_ident : com_runtime::GUID = $clsid_guid_tokens;
    ).unwrap();
    push( Annotatable::Item( clsid_const ) );

    let com_ptr = quote_item!(
        cx,
        #[repr(C)]
        struct $com_ptr_ident {
            __vtable : &'static $vtable_ident,
            rc : u32,
            data: $struct_name
        } ).unwrap();
    push( Annotatable::Item( com_ptr ) );

    let com_ptr_impl = quote_item!(
        cx,
        impl $com_ptr_ident {
            #[allow(dead_code)]
            fn new() -> $com_ptr_ident {
                $com_ptr_ident {
                    __vtable: &$vtable_instance_name,
                    rc: 0,
                    data: $struct_name::new()
                }
            }
            #[allow(dead_code)]
            fn wrap( data : $struct_name ) -> $com_ptr_ident {
                $com_ptr_ident {
                    __vtable: &$vtable_instance_name,
                    rc: 0,
                    data: data
                }
            }
        } ).unwrap();
    push( Annotatable::Item( com_ptr_impl ) );

    let com_ptr_deref = quote_item!(
        cx,
        impl std::ops::Deref for $com_ptr_ident {
            type Target = $struct_name;
            fn deref(&self) -> &$struct_name {
                &self.data
            }
        } ).unwrap();
    push( Annotatable::Item( com_ptr_deref ) );

    let com_ptr_derefmut = quote_item!(
        cx,
        impl std::ops::DerefMut for $com_ptr_ident {
            fn deref_mut(&mut self) -> &mut $struct_name {
                &mut self.data
            }
        } ).unwrap();
    push( Annotatable::Item( com_ptr_derefmut ) );

    Ok(())
}

pub fn expand_com_visible(
    cx: &mut ExtCtxt,
    sp: Span,
    mi: &MetaItem,
    item: &Annotatable,
    push: &mut FnMut( Annotatable )
) {
    if let Err( err ) = try_expand_com_visible( cx, sp, mi, item, push ) {
        cx.span_err( mi.span, err );
    }
}

pub fn try_expand_com_library(
    cx: &mut ExtCtxt,
    _sp: Span,
    mi: &MetaItem,
    _item: &Annotatable,
    push: &mut FnMut( Annotatable )
) -> Result< (), &'static str >
{
    // Get the annotated item.
    /*
    let impl_item = match item {
        &Annotatable::Item( ref itf ) => itf,
        _ => return Err( "[com_library(...)] must be applied to the crate" )
    };

    let mod_item = match impl_item.node {
        ItemKind::Mod( ref m ) => m,
        _ => return Err( "[com_library(...)] must be applied to the crate" )
    };
    */

    let params = match mi.node {
        MetaItemKind::List( ref v ) => v,
        _ => return Err( "[com_library(...)] needs visible structs as parameters." )
    };

    let mut match_arms : Vec<Vec<TokenTree>> = vec![];
    for p in params {

        let class_name = match p.node {
            NestedMetaItemKind::MetaItem( ref l ) => &l.name,
            _ => return Err( "[com_visible(clsid : &str) must have CLSID as a parameter." )
        };

        let class_ident = Ident::from_str( &class_name.as_str() );
        let clsid_name = get_clsid_ident( cx, &class_ident );
        let coclass_ptr = get_com_ptr_ident( cx, &class_ident );
        match_arms.push( quote_tokens!( cx,
            $clsid_name => {
                let mut b = Box::new( $coclass_ptr::new() );
                b.rc += 1;
                Box::into_raw( b ) as com_runtime::ComPtr
            },
        ) );
    }


    let vtable_instance_ident = Ident::from_str( "__ClassFactory_vtable_instance" );
    let create_instance_ident = Ident::from_str( "__ClassFactory_create_instance" );
    let vtable_instance = quote_item!(
        cx,
        #[allow(non_upper_case_globals)]
        const $vtable_instance_ident : com_runtime::__ClassFactory_vtable =
                com_runtime::__ClassFactory_vtable {
                    __base : com_runtime::__IUnknown_vtable {
                        query_interface : com_runtime::ClassFactory::query_interface,
                        add_ref : com_runtime::ClassFactory::add_ref,
                        release : com_runtime::ClassFactory::release,
                    },
                    create_instance: $create_instance_ident,
                    lock_server : com_runtime::ClassFactory::lock_server,
                };
    ).unwrap();
    push( Annotatable::Item( vtable_instance ) );

    let create_instance_def = quote_item!(
        cx,
        #[allow(non_snake_case)]
        #[allow(dead_code)]
        pub unsafe extern "stdcall" fn $create_instance_ident(
            self_void: com_runtime::ComPtr,
            _outer : com_runtime::ComPtr,
            _iid : com_runtime::REFIID,
            out : *mut com_runtime::ComPtr
        ) -> u32
        {
            let self_ptr : *mut com_runtime::ClassFactory = std::mem::transmute( self_void );

            #[allow(non_upper_case_globals)]
            let out_ptr = match *(*self_ptr).clsid {
                $match_arms
                _ => {
                    return com_runtime::E_NOINTERFACE
                }
            };
            *out = out_ptr;
            com_runtime::S_OK
        } ).unwrap();
    push( Annotatable::Item( create_instance_def ) );

    let dll_get_class_object = quote_item!(
        cx,
        #[no_mangle]
        #[allow(non_snake_case)]
        #[allow(dead_code)]
        pub unsafe extern "stdcall" fn DllGetClassObject(
            rclsid : com_runtime::REFCLSID,
            _riid : com_runtime::REFIID,
            pout : *mut com_runtime::ComPtr
        ) -> u32
        {
            let classFactory = Box::new( com_runtime::ClassFactory {
                __vtable : &$vtable_instance_ident,
                clsid : rclsid,
                rc : 1
            } );
            let ptrClassFactory = Box::into_raw( classFactory );
            *pout = ptrClassFactory as com_runtime::ComPtr;
            com_runtime::S_OK
        }
    ).unwrap();
    push( Annotatable::Item( dll_get_class_object ) );

    Ok(())
}

pub fn expand_com_library(
    cx: &mut ExtCtxt,
    sp: Span,
    mi: &MetaItem,
    item: &Annotatable,
    push: &mut FnMut( Annotatable )
) {
    if let Err( err ) = try_expand_com_library( cx, sp, mi, item, push ) {
        cx.span_err( mi.span, err );
    }
}

#[plugin_registrar]
pub fn registrar( reg: &mut Registry ) {
    reg.register_syntax_extension(
            Symbol::intern("com_visible"),
            MultiDecorator( Box::new( expand_com_visible ) ) );
    reg.register_syntax_extension(
            Symbol::intern("com_library"),
            MultiDecorator( Box::new( expand_com_library ) ) );
}

#[allow(dead_code)]
fn print_item( i : &P<Item> ) {
    if let Some( ref tt ) = i.tokens {
        println!( "{}", tt );
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
