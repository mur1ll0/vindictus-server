use serde::Deserialize;
use vindictus_server::domain::{
    item_binding::BindingInfo,
    item_catalog::EffectiveItem,
    item_preparation::{
        GiveRequest, PreparationBound, RequestProperty, StackAttribute, is_stackable, prepare,
    },
};
#[derive(Deserialize)]
struct Fixture {
    cases: Cases,
}
#[derive(Deserialize)]
struct Cases {
    pairs: Vec<Pair>,
    batches: Vec<Batch>,
}
#[derive(Deserialize)]
struct Pair {
    left: Input,
    right: Input,
    stackable: bool,
}
#[derive(Deserialize)]
struct Batch {
    requests: Vec<Input>,
    result: Option<Vec<Output>>,
}
#[derive(Deserialize, Debug, PartialEq)]
struct Output {
    id: i32,
    count: i32,
    before: Option<i64>,
}
#[derive(Deserialize)]
struct Class {
    name: String,
    max_stack: i32,
    storage: u8,
    equipped: bool,
}
#[derive(Deserialize)]
struct Attribute {
    key: String,
    value: Option<String>,
    arg: i32,
    arg2: i32,
}
#[derive(Deserialize)]
struct Input {
    id: i32,
    item: Option<Class>,
    bound: bool,
    expires: Option<i64>,
    attrs: Vec<Attribute>,
    colors: [i32; 3],
    reduced: i32,
    bonus: i32,
    count: i32,
    before: Option<i64>,
}
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/framework-item-preparation.json")).unwrap()
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
struct OwnedAttribute {
    key: Vec<u8>,
    value: Option<Vec<u8>>,
    arg: i32,
    arg2: i32,
}
struct Owned {
    class: Option<Vec<u8>>,
    attrs: Vec<OwnedAttribute>,
}
impl Owned {
    fn new(input: &Input) -> Self {
        Self {
            class: input.item.as_ref().map(|v| utf16(&v.name)),
            attrs: input
                .attrs
                .iter()
                .map(|a| OwnedAttribute {
                    key: utf16(&a.key),
                    value: a.value.as_deref().map(utf16),
                    arg: a.arg,
                    arg2: a.arg2,
                })
                .collect(),
        }
    }
    fn attributes(&self) -> Vec<StackAttribute<'_>> {
        self.attrs
            .iter()
            .map(|a| StackAttribute {
                key: &a.key,
                value: a.value.as_deref(),
                arg: a.arg,
                arg2: a.arg2,
            })
            .collect()
    }
    fn property<'a>(
        &'a self,
        input: &Input,
        attrs: &'a [StackAttribute<'a>],
    ) -> RequestProperty<'a> {
        RequestProperty {
            item: input.item.as_ref().map(|i| EffectiveItem {
                binding: BindingInfo {
                    item_class: self.class.as_deref().unwrap(),
                    equippable: i.equipped,
                    bind: None,
                    trade_on_bind: None,
                },
                max_stack: i.max_stack,
                storage: i.storage,
                source_index: 0,
            }),
            character_bound: input.bound,
            expire_ticks: input.expires,
            attributes: attrs,
            colors: input.colors,
            reduced_durability: input.reduced,
            max_durability_bonus: input.bonus,
        }
    }
}
#[test]
fn stack_equivalence_matches_recovered_framework_on_both_architectures() {
    for (case, pair) in fixture().cases.pairs.into_iter().enumerate() {
        let l = Owned::new(&pair.left);
        let r = Owned::new(&pair.right);
        let la = l.attributes();
        let ra = r.attributes();
        assert_eq!(
            is_stackable(&l.property(&pair.left, &la), &r.property(&pair.right, &ra)).unwrap(),
            pair.stackable,
            "pair {case}"
        );
    }
}
#[test]
fn factory_batches_match_framework_order_identity_rejection_and_wrapping() {
    for (case, batch) in fixture().cases.batches.into_iter().enumerate() {
        let owned: Vec<_> = batch.requests.iter().map(Owned::new).collect();
        let attrs: Vec<_> = owned.iter().map(Owned::attributes).collect();
        let props: Vec<_> = owned
            .iter()
            .zip(&batch.requests)
            .zip(&attrs)
            .map(|((o, i), a)| o.property(i, a))
            .collect();
        let requests: Vec<_> = props
            .iter()
            .zip(&batch.requests)
            .map(|(p, i)| GiveRequest {
                property: p,
                count: i.count,
                before_item_id: i.before,
            })
            .collect();
        let actual = prepare(&requests).unwrap().map(|items| {
            items
                .iter()
                .map(|v| {
                    let index = props
                        .iter()
                        .position(|p| std::ptr::eq(p, v.property))
                        .expect("entire original property retained");
                    Output {
                        id: batch.requests[index].id,
                        count: v.count,
                        before: v.before_item_id,
                    }
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(actual, batch.result, "batch {case}");
    }
}
#[test]
fn unbounded_preparation_is_rejected_without_partial_output() {
    let raw = Input {
        id: 0,
        item: Some(Class {
            name: "bound".into(),
            max_stack: 1,
            storage: 1,
            equipped: false,
        }),
        bound: false,
        expires: None,
        attrs: vec![],
        colors: [1, 2, 3],
        reduced: 4,
        bonus: 5,
        count: 1,
        before: None,
    };
    let owned = Owned::new(&raw);
    let attrs = owned.attributes();
    let mut prop = owned.property(&raw, &attrs);
    assert!(matches!(
        prepare(&[GiveRequest {
            property: &prop,
            count: 4097,
            before_item_id: None
        }]),
        Err(PreparationBound)
    ));
    let requests: Vec<_> = (0..4097)
        .map(|_| GiveRequest {
            property: &prop,
            count: 1,
            before_item_id: None,
        })
        .collect();
    assert!(matches!(prepare(&requests), Err(PreparationBound)));
    prop.expire_ticks = Some(-1);
    assert!(matches!(
        prepare(&[GiveRequest {
            property: &prop,
            count: 1,
            before_item_id: None
        }]),
        Err(PreparationBound)
    ));
    prop.expire_ticks = None;
    let bad = [StackAttribute {
        key: &[0],
        value: None,
        arg: 0,
        arg2: 0,
    }];
    prop.attributes = &bad;
    assert_eq!(is_stackable(&prop, &prop), Err(PreparationBound));
    let duplicate = [
        StackAttribute {
            key: &[],
            value: None,
            arg: 0,
            arg2: 0,
        },
        StackAttribute {
            key: &[],
            value: None,
            arg: 0,
            arg2: 0,
        },
    ];
    prop.attributes = &duplicate;
    assert_eq!(is_stackable(&prop, &prop), Err(PreparationBound));
}

#[test]
fn quadratic_distinct_request_work_is_refused_explicitly() {
    let raw = Input {
        id: 0,
        item: Some(Class {
            name: "bound".into(),
            max_stack: 1,
            storage: 1,
            equipped: true,
        }),
        bound: false,
        expires: None,
        attrs: vec![],
        colors: [0; 3],
        reduced: 0,
        bonus: 0,
        count: 1,
        before: None,
    };
    let owned = Owned::new(&raw);
    let attrs = owned.attributes();
    let prop = owned.property(&raw, &attrs);
    let requests: Vec<_> = (0..1500)
        .map(|_| GiveRequest {
            property: &prop,
            count: 1,
            before_item_id: None,
        })
        .collect();
    assert!(matches!(prepare(&requests), Err(PreparationBound)));
}
