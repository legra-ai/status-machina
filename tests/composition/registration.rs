use super::*;

#[test]
fn registered_instances_are_scoped_and_duplicate_routes_are_rejected() {
    let source_a = address("tenant-a", 10);
    let source_b = address("tenant-b", 11);
    let destination = address("tenant-a", 12);
    let source_definition = source_definition();
    let source_a_instance = source_definition
        .instantiate(source_a.instance_id(), ())
        .unwrap();
    let source_b_instance = source_definition
        .instantiate(source_b.instance_id(), ())
        .unwrap();
    let destination_definition = destination_definition();
    let destination_instance = destination_definition
        .instantiate(destination.instance_id(), ())
        .unwrap();
    let registry = InstanceRegistry::builder()
        .instance(source_a.clone(), &source_a_instance)
        .unwrap()
        .instance(destination.clone(), &destination_instance)
        .unwrap()
        .instance(source_b.clone(), &source_b_instance)
        .unwrap()
        .build();
    let route = Route::builder(source_a.clone(), EventId::try_new("notification").unwrap())
        .to(destination.clone())
        .as_event(EventId::try_new("receive-notification").unwrap())
        .build()
        .unwrap();
    let duplicate = route.clone();
    let result = Composition::<Notification>::builder(registry.clone(), registry)
        .route(route)
        .unwrap()
        .route(duplicate);
    assert!(result.is_err());

    let isolated = Composition::<Notification>::builder(
        InstanceRegistry::builder()
            .instance(source_a.clone(), &source_a_instance)
            .unwrap()
            .instance(destination.clone(), &destination_instance)
            .unwrap()
            .instance(source_b.clone(), &source_b_instance)
            .unwrap()
            .build(),
        InstanceRegistry::builder()
            .instance(source_a.clone(), &source_a_instance)
            .unwrap()
            .instance(destination.clone(), &destination_instance)
            .unwrap()
            .instance(source_b.clone(), &source_b_instance)
            .unwrap()
            .build(),
    )
    .route(
        Route::builder(source_a.clone(), EventId::try_new("notification").unwrap())
            .to(destination.clone())
            .as_event(EventId::try_new("receive-notification").unwrap())
            .build()
            .unwrap(),
    )
    .unwrap()
    .build()
    .unwrap();
    let result = isolated
        .route_batch(
            &source_b,
            vec![source_output_for(source_b.instance_id())],
            &CapabilitySnapshot::empty(),
        )
        .unwrap();
    assert!(matches!(
        &result.items()[0],
        RoutedItem::Unmatched { source, .. } if source == &source_b
    ));
}
