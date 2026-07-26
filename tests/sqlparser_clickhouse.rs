// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

#![warn(clippy::all)]
//! Test SQL syntax specific to ClickHouse.

#[macro_use]
mod test_utils;

use helpers::attached_token::AttachedToken;
use sqlparser::tokenizer::Span;
use test_utils::*;

use sqlparser::ast::Expr::{BinaryOp, Identifier};
use sqlparser::ast::SelectItem::UnnamedExpr;
use sqlparser::ast::TableFactor::Table;
use sqlparser::ast::Value::Boolean;
use sqlparser::ast::*;
use sqlparser::dialect::ClickHouseDialect;
use sqlparser::dialect::GenericDialect;
use sqlparser::dialect::MySqlDialect;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::ParserError::ParserError;

#[test]
fn parse_map_access_expr() {
    let sql = r#"SELECT string_values[indexOf(string_names, 'endpoint')] FROM foos WHERE id = 'test' AND string_value[indexOf(string_name, 'app')] <> 'foo'"#;
    let select = clickhouse().verified_only_select(sql);
    assert_eq!(
        Select {
            select_token: AttachedToken::empty(),
            optimizer_hints: vec![],
            distinct: None,
            select_modifiers: None,
            top: None,
            top_before_distinct: false,
            projection: vec![UnnamedExpr(Expr::CompoundFieldAccess {
                root: Box::new(Identifier(Ident {
                    value: "string_values".to_string(),
                    quote_style: None,
                    span: Span::empty(),
                })),
                access_chain: vec![AccessExpr::Subscript(Subscript::Index {
                    index: call(
                        "indexOf",
                        [
                            Expr::Identifier(Ident::new("string_names")),
                            Expr::value(Value::SingleQuotedString("endpoint".to_string()))
                        ]
                    ),
                })],
            })],
            exclude: None,
            into: None,
            from: vec![TableWithJoins {
                relation: table_from_name(ObjectName::from(vec![Ident::new("foos")])),
                joins: vec![],
                array_joins: vec![],
            }],
            lateral_views: vec![],
            prewhere: None,
            selection: Some(BinaryOp {
                left: Box::new(BinaryOp {
                    left: Box::new(Identifier(Ident::new("id"))),
                    op: BinaryOperator::Eq,
                    right: Box::new(Expr::value(Value::SingleQuotedString("test".to_string()))),
                }),
                op: BinaryOperator::And,
                right: Box::new(BinaryOp {
                    left: Box::new(Expr::CompoundFieldAccess {
                        root: Box::new(Identifier(Ident::new("string_value"))),
                        access_chain: vec![AccessExpr::Subscript(Subscript::Index {
                            index: call(
                                "indexOf",
                                [
                                    Expr::Identifier(Ident::new("string_name")),
                                    Expr::value(Value::SingleQuotedString("app".to_string()))
                                ]
                            ),
                        })],
                    }),
                    op: BinaryOperator::NotEq,
                    right: Box::new(Expr::value(Value::SingleQuotedString("foo".to_string()))),
                }),
            }),
            group_by: GroupByExpr::Expressions(vec![], vec![]),
            cluster_by: vec![],
            distribute_by: vec![],
            sort_by: vec![],
            having: None,
            named_window: vec![],
            window_before_qualify: false,
            qualify: None,
            value_table_mode: None,
            connect_by: vec![],
            flavor: SelectFlavor::Standard,
        },
        select
    );
}

#[test]
fn parse_array_expr() {
    let sql = "SELECT ['1', '2'] FROM test";
    let select = clickhouse().verified_only_select(sql);
    assert_eq!(
        &Expr::Array(Array {
            elem: vec![
                Expr::value(Value::SingleQuotedString("1".to_string())),
                Expr::value(Value::SingleQuotedString("2".to_string())),
            ],
            named: false,
        }),
        expr_from_projection(only(&select.projection))
    )
}

#[test]
fn parse_array_fn() {
    let sql = "SELECT array(x1, x2) FROM foo";
    let select = clickhouse().verified_only_select(sql);
    assert_eq!(
        &call(
            "array",
            [
                Expr::Identifier(Ident::new("x1")),
                Expr::Identifier(Ident::new("x2"))
            ]
        ),
        expr_from_projection(only(&select.projection))
    );
}

#[test]
fn parse_kill() {
    let stmt = clickhouse().verified_stmt("KILL MUTATION 5");
    assert_eq!(
        stmt,
        Statement::Kill {
            modifier: Some(KillType::Mutation),
            id: 5,
        }
    );
}

#[test]
fn parse_delimited_identifiers() {
    // check that quoted identifiers in any position remain quoted after serialization
    let select = clickhouse().verified_only_select(
        r#"SELECT "alias"."bar baz", "myfun"(), "simple id" AS "column alias" FROM "a table" AS "alias""#,
    );
    // check FROM
    match only(select.from).relation {
        TableFactor::Table {
            name,
            alias,
            args,
            with_hints,
            version,
            ..
        } => {
            assert_eq!(
                ObjectName::from(vec![Ident::with_quote('"', "a table")]),
                name
            );
            assert_eq!(Ident::with_quote('"', "alias"), alias.unwrap().name);
            assert!(args.is_none());
            assert!(with_hints.is_empty());
            assert!(version.is_none());
        }
        _ => panic!("Expecting TableFactor::Table"),
    }
    // check SELECT
    assert_eq!(3, select.projection.len());
    assert_eq!(
        &Expr::CompoundIdentifier(vec![
            Ident::with_quote('"', "alias"),
            Ident::with_quote('"', "bar baz"),
        ]),
        expr_from_projection(&select.projection[0]),
    );
    assert_eq!(
        &Expr::Function(Function {
            name: ObjectName::from(vec![Ident::with_quote('"', "myfun")]),
            uses_odbc_syntax: false,
            parameters: FunctionArguments::None,
            args: FunctionArguments::List(FunctionArgumentList {
                duplicate_treatment: None,
                args: vec![],
                clauses: vec![],
            }),
            null_treatment: None,
            filter: None,
            over: None,
            within_group: vec![],
        }),
        expr_from_projection(&select.projection[1]),
    );
    match &select.projection[2] {
        SelectItem::ExprWithAlias { expr, alias } => {
            assert_eq!(&Expr::Identifier(Ident::with_quote('"', "simple id")), expr);
            assert_eq!(&Ident::with_quote('"', "column alias"), alias);
        }
        _ => panic!("Expected ExprWithAlias"),
    }

    clickhouse().verified_stmt(r#"CREATE TABLE "foo" ("bar" "int")"#);
    clickhouse().verified_stmt(r#"ALTER TABLE foo ADD CONSTRAINT "bar" PRIMARY KEY (baz)"#);
    //TODO verified_stmt(r#"UPDATE foo SET "bar" = 5"#);
}

#[test]
fn parse_create_table() {
    clickhouse().verified_stmt(r#"CREATE TABLE "x" ("a" "int") ENGINE = MergeTree ORDER BY ("x")"#);
    clickhouse().verified_stmt(r#"CREATE TABLE "x" ("a" "int") ENGINE = MergeTree ORDER BY "x""#);
    clickhouse().verified_stmt(
        r#"CREATE TABLE "x" ("a" "int") ENGINE = MergeTree ORDER BY "x" AS SELECT * FROM "t" WHERE true"#,
    );
    clickhouse().one_statement_parses_to(
        "CREATE TABLE x (a int) ENGINE = MergeTree() ORDER BY a",
        "CREATE TABLE x (a INT) ENGINE = MergeTree ORDER BY a",
    );
}

#[test]
fn parse_create_table_partition_by_after_order_by() {
    // ClickHouse DDL places PARTITION BY after ORDER BY.
    // MergeTree() is canonicalized to MergeTree and type names are uppercased.
    clickhouse().one_statement_parses_to(
        concat!(
            "CREATE TABLE IF NOT EXISTS \"MyTable\" (`col1` Int64, `col2` Int32) ",
            "ENGINE = MergeTree() ",
            "PRIMARY KEY (toDate(toDateTime(`col2`)), `col1`, `col2`) ",
            "ORDER BY (toDate(toDateTime(`col2`)), `col1`, `col2`) ",
            "PARTITION BY col1 % 64"
        ),
        concat!(
            "CREATE TABLE IF NOT EXISTS \"MyTable\" (`col1` INT64, `col2` Int32) ",
            "ENGINE = MergeTree ",
            "PRIMARY KEY (toDate(toDateTime(`col2`)), `col1`, `col2`) ",
            "ORDER BY (toDate(toDateTime(`col2`)), `col1`, `col2`) ",
            "PARTITION BY col1 % 64"
        ),
    );

    // PARTITION BY after ORDER BY works with both ClickHouseDialect and GenericDialect
    clickhouse_and_generic()
        .verified_stmt("CREATE TABLE t (a INT) ENGINE = MergeTree ORDER BY a PARTITION BY a");

    // Arithmetic expression in PARTITION BY (roundtrip)
    clickhouse_and_generic()
        .verified_stmt("CREATE TABLE t (a INT) ENGINE = MergeTree ORDER BY a PARTITION BY a % 64");

    // AST: partition_by is populated with the correct expression
    match clickhouse_and_generic()
        .verified_stmt("CREATE TABLE t (a INT) ENGINE = MergeTree ORDER BY a PARTITION BY a % 64")
    {
        Statement::CreateTable(CreateTable { partition_by, .. }) => {
            assert_eq!(
                partition_by,
                Some(Box::new(BinaryOp {
                    left: Box::new(Identifier(Ident::new("a"))),
                    op: BinaryOperator::Modulo,
                    right: Box::new(Expr::Value(
                        Value::Number("64".parse().unwrap(), false).with_empty_span(),
                    )),
                }))
            );
        }
        _ => unreachable!(),
    }

    // Function call expression in PARTITION BY (ClickHouse-specific function)
    clickhouse().verified_stmt(
        "CREATE TABLE t (d DATE) ENGINE = MergeTree ORDER BY d PARTITION BY toYYYYMM(d)",
    );

    // Negative: PARTITION BY with no expression should fail
    clickhouse_and_generic()
        .parse_sql_statements("CREATE TABLE t (a INT) ENGINE = MergeTree ORDER BY a PARTITION BY")
        .expect_err("PARTITION BY with no expression should fail");
}

#[test]
fn parse_insert_into_function() {
    clickhouse().verified_stmt(r#"INSERT INTO TABLE FUNCTION remote('localhost', default.simple_table) VALUES (100, 'inserted via remote()')"#);
    clickhouse().verified_stmt(r#"INSERT INTO FUNCTION remote('localhost', default.simple_table) VALUES (100, 'inserted via remote()')"#);
}

#[test]
fn parse_alter_table_attach_and_detach_partition() {
    for operation in &["ATTACH", "DETACH"] {
        match clickhouse_and_generic()
            .verified_stmt(format!("ALTER TABLE t0 {operation} PARTITION part").as_str())
        {
            Statement::AlterTable(alter_table) => {
                pretty_assertions::assert_eq!("t0", alter_table.name.to_string());
                pretty_assertions::assert_eq!(
                    alter_table.operations[0],
                    if operation == &"ATTACH" {
                        AlterTableOperation::AttachPartition {
                            partition: Partition::Expr(Identifier(Ident::new("part"))),
                        }
                    } else {
                        AlterTableOperation::DetachPartition {
                            partition: Partition::Expr(Identifier(Ident::new("part"))),
                        }
                    }
                );
            }
            _ => unreachable!(),
        }

        match clickhouse_and_generic()
            .verified_stmt(format!("ALTER TABLE t1 {operation} PART part").as_str())
        {
            Statement::AlterTable(AlterTable {
                name, operations, ..
            }) => {
                pretty_assertions::assert_eq!("t1", name.to_string());
                pretty_assertions::assert_eq!(
                    operations[0],
                    if operation == &"ATTACH" {
                        AlterTableOperation::AttachPartition {
                            partition: Partition::Part(Identifier(Ident::new("part"))),
                        }
                    } else {
                        AlterTableOperation::DetachPartition {
                            partition: Partition::Part(Identifier(Ident::new("part"))),
                        }
                    }
                );
            }
            _ => unreachable!(),
        }

        // negative cases
        assert_eq!(
            clickhouse_and_generic()
                .parse_sql_statements(format!("ALTER TABLE t0 {operation} PARTITION").as_str())
                .unwrap_err(),
            ParserError("Expected: an expression, found: EOF".to_string())
        );
        assert_eq!(
            clickhouse_and_generic()
                .parse_sql_statements(format!("ALTER TABLE t0 {operation} PART").as_str())
                .unwrap_err(),
            ParserError("Expected: an expression, found: EOF".to_string())
        );
    }
}

#[test]
fn parse_alter_table_add_projection() {
    match clickhouse_and_generic().verified_stmt(concat!(
        "ALTER TABLE t0 ADD PROJECTION IF NOT EXISTS my_name",
        " (SELECT a, b GROUP BY a ORDER BY b)",
    )) {
        Statement::AlterTable(AlterTable {
            name, operations, ..
        }) => {
            assert_eq!(name, ObjectName::from(vec!["t0".into()]));
            assert_eq!(1, operations.len());
            assert_eq!(
                operations[0],
                AlterTableOperation::AddProjection {
                    if_not_exists: true,
                    name: "my_name".into(),
                    select: ProjectionSelect {
                        projection: vec![
                            UnnamedExpr(Identifier(Ident::new("a"))),
                            UnnamedExpr(Identifier(Ident::new("b"))),
                        ],
                        group_by: Some(GroupByExpr::Expressions(
                            vec![Identifier(Ident::new("a"))],
                            vec![]
                        )),
                        order_by: Some(OrderBy {
                            kind: OrderByKind::Expressions(vec![OrderByExpr {
                                expr: Identifier(Ident::new("b")),
                                options: OrderByOptions {
                                    sort: None,
                                    nulls_first: None,
                                },
                                with_fill: None,
                            }]),
                            interpolate: None,
                        }),
                    }
                }
            )
        }
        _ => unreachable!(),
    }

    // leave out IF NOT EXISTS is allowed
    clickhouse_and_generic()
        .verified_stmt("ALTER TABLE t0 ADD PROJECTION my_name (SELECT a, b GROUP BY a ORDER BY b)");
    // leave out GROUP BY is allowed
    clickhouse_and_generic()
        .verified_stmt("ALTER TABLE t0 ADD PROJECTION my_name (SELECT a, b ORDER BY b)");
    // leave out ORDER BY is allowed
    clickhouse_and_generic()
        .verified_stmt("ALTER TABLE t0 ADD PROJECTION my_name (SELECT a, b GROUP BY a)");

    // missing select query is not allowed
    assert_eq!(
        clickhouse_and_generic()
            .parse_sql_statements("ALTER TABLE t0 ADD PROJECTION my_name")
            .unwrap_err(),
        ParserError("Expected: (, found: EOF".to_string())
    );
    assert_eq!(
        clickhouse_and_generic()
            .parse_sql_statements("ALTER TABLE t0 ADD PROJECTION my_name ()")
            .unwrap_err(),
        ParserError("Expected: SELECT, found: )".to_string())
    );
    assert_eq!(
        clickhouse_and_generic()
            .parse_sql_statements("ALTER TABLE t0 ADD PROJECTION my_name (SELECT)")
            .unwrap_err(),
        ParserError("Expected: an expression, found: )".to_string())
    );
}

#[test]
fn parse_alter_table_drop_projection() {
    match clickhouse_and_generic().verified_stmt("ALTER TABLE t0 DROP PROJECTION IF EXISTS my_name")
    {
        Statement::AlterTable(AlterTable {
            name, operations, ..
        }) => {
            assert_eq!(name, ObjectName::from(vec!["t0".into()]));
            assert_eq!(1, operations.len());
            assert_eq!(
                operations[0],
                AlterTableOperation::DropProjection {
                    if_exists: true,
                    name: "my_name".into(),
                }
            )
        }
        _ => unreachable!(),
    }
    // allow to skip `IF EXISTS`
    clickhouse_and_generic().verified_stmt("ALTER TABLE t0 DROP PROJECTION my_name");

    assert_eq!(
        clickhouse_and_generic()
            .parse_sql_statements("ALTER TABLE t0 DROP PROJECTION")
            .unwrap_err(),
        ParserError("Expected: identifier, found: EOF".to_string())
    );
}

#[test]
fn parse_alter_table_clear_and_materialize_projection() {
    for keyword in ["CLEAR", "MATERIALIZE"] {
        match clickhouse_and_generic().verified_stmt(
            format!("ALTER TABLE t0 {keyword} PROJECTION IF EXISTS my_name IN PARTITION p0",)
                .as_str(),
        ) {
            Statement::AlterTable(AlterTable {
                name, operations, ..
            }) => {
                assert_eq!(name, ObjectName::from(vec!["t0".into()]));
                assert_eq!(1, operations.len());
                assert_eq!(
                    operations[0],
                    if keyword == "CLEAR" {
                        AlterTableOperation::ClearProjection {
                            if_exists: true,
                            name: "my_name".into(),
                            partition: Some(Ident::new("p0")),
                        }
                    } else {
                        AlterTableOperation::MaterializeProjection {
                            if_exists: true,
                            name: "my_name".into(),
                            partition: Some(Ident::new("p0")),
                        }
                    }
                )
            }
            _ => unreachable!(),
        }
        // allow to skip `IF EXISTS`
        clickhouse_and_generic().verified_stmt(
            format!("ALTER TABLE t0 {keyword} PROJECTION my_name IN PARTITION p0",).as_str(),
        );
        // allow to skip `IN PARTITION partition_name`
        clickhouse_and_generic()
            .verified_stmt(format!("ALTER TABLE t0 {keyword} PROJECTION my_name",).as_str());

        assert_eq!(
            clickhouse_and_generic()
                .parse_sql_statements(format!("ALTER TABLE t0 {keyword} PROJECTION",).as_str())
                .unwrap_err(),
            ParserError("Expected: identifier, found: EOF".to_string())
        );

        assert_eq!(
            clickhouse_and_generic()
                .parse_sql_statements(
                    format!("ALTER TABLE t0 {keyword} PROJECTION my_name IN PARTITION",).as_str()
                )
                .unwrap_err(),
            ParserError("Expected: identifier, found: EOF".to_string())
        );

        assert_eq!(
            clickhouse_and_generic()
                .parse_sql_statements(
                    format!("ALTER TABLE t0 {keyword} PROJECTION my_name IN",).as_str()
                )
                .unwrap_err(),
            ParserError("Expected: end of statement, found: IN".to_string())
        );
    }
}

#[test]
fn parse_optimize_table() {
    clickhouse_and_generic().verified_stmt("OPTIMIZE TABLE t0");
    clickhouse_and_generic().verified_stmt("OPTIMIZE TABLE db.t0");
    clickhouse_and_generic().verified_stmt("OPTIMIZE TABLE t0 ON CLUSTER 'cluster'");
    clickhouse_and_generic().verified_stmt("OPTIMIZE TABLE t0 ON CLUSTER 'cluster' FINAL");
    clickhouse_and_generic().verified_stmt("OPTIMIZE TABLE t0 FINAL DEDUPLICATE");
    clickhouse_and_generic().verified_stmt("OPTIMIZE TABLE t0 DEDUPLICATE");
    clickhouse_and_generic().verified_stmt("OPTIMIZE TABLE t0 DEDUPLICATE BY id");
    clickhouse_and_generic().verified_stmt("OPTIMIZE TABLE t0 FINAL DEDUPLICATE BY id");
    clickhouse_and_generic()
        .verified_stmt("OPTIMIZE TABLE t0 PARTITION tuple('2023-04-22') DEDUPLICATE BY id");
    match clickhouse_and_generic().verified_stmt(
        "OPTIMIZE TABLE t0 ON CLUSTER cluster PARTITION ID '2024-07' FINAL DEDUPLICATE BY id",
    ) {
        Statement::OptimizeTable {
            name,
            on_cluster,
            partition,
            include_final,
            deduplicate,
            ..
        } => {
            assert_eq!(name.to_string(), "t0");
            assert_eq!(on_cluster, Some(Ident::new("cluster")));
            assert_eq!(
                partition,
                Some(Partition::Identifier(Ident::with_quote('\'', "2024-07")))
            );
            assert!(include_final);
            assert_eq!(
                deduplicate,
                Some(Deduplicate::ByExpression(Identifier(Ident::new("id"))))
            );
        }
        _ => unreachable!(),
    }

    // negative cases
    assert_eq!(
        clickhouse_and_generic()
            .parse_sql_statements("OPTIMIZE TABLE t0 DEDUPLICATE BY")
            .unwrap_err(),
        ParserError("Expected: an expression, found: EOF".to_string())
    );
    assert_eq!(
        clickhouse_and_generic()
            .parse_sql_statements("OPTIMIZE TABLE t0 PARTITION")
            .unwrap_err(),
        ParserError("Expected: an expression, found: EOF".to_string())
    );
    assert_eq!(
        clickhouse_and_generic()
            .parse_sql_statements("OPTIMIZE TABLE t0 PARTITION ID")
            .unwrap_err(),
        ParserError("Expected: identifier, found: EOF".to_string())
    );
}

fn column_def(name: Ident, data_type: DataType) -> ColumnDef {
    ColumnDef {
        name,
        data_type,
        options: vec![],
    }
}

#[test]
fn parse_clickhouse_data_types() {
    let sql = concat!(
        "CREATE TABLE table (",
        "a1 UInt8, a2 UInt16, a3 UInt32, a4 UInt64, a5 UInt128, a6 UInt256,",
        " b1 Int8, b2 Int16, b3 Int32, b4 Int64, b5 Int128, b6 Int256,",
        " c1 Float32, c2 Float64,",
        " d1 Date32, d2 DateTime64(3), d3 DateTime64(3, 'UTC'),",
        " e1 FixedString(255),",
        " f1 LowCardinality(Int32)",
        ") ORDER BY (a1)",
    );
    // ClickHouse has a case-sensitive definition of data type, but canonical representation is not
    let canonical_sql = sql
        .replace(" Int8", " INT8")
        .replace(" Int64", " INT64")
        .replace(" Float64", " FLOAT64");

    match clickhouse_and_generic().one_statement_parses_to(sql, &canonical_sql) {
        Statement::CreateTable(CreateTable { name, columns, .. }) => {
            assert_eq!(name, ObjectName::from(vec!["table".into()]));
            assert_eq!(
                columns,
                vec![
                    column_def("a1".into(), DataType::UInt8),
                    column_def("a2".into(), DataType::UInt16),
                    column_def("a3".into(), DataType::UInt32),
                    column_def("a4".into(), DataType::UInt64),
                    column_def("a5".into(), DataType::UInt128),
                    column_def("a6".into(), DataType::UInt256),
                    column_def("b1".into(), DataType::Int8(None)),
                    column_def("b2".into(), DataType::Int16),
                    column_def("b3".into(), DataType::Int32),
                    column_def("b4".into(), DataType::Int64),
                    column_def("b5".into(), DataType::Int128),
                    column_def("b6".into(), DataType::Int256),
                    column_def("c1".into(), DataType::Float32),
                    column_def("c2".into(), DataType::Float64),
                    column_def("d1".into(), DataType::Date32),
                    column_def("d2".into(), DataType::Datetime64(3, None)),
                    column_def("d3".into(), DataType::Datetime64(3, Some("UTC".into()))),
                    column_def("e1".into(), DataType::FixedString(255)),
                    column_def(
                        "f1".into(),
                        DataType::LowCardinality(Box::new(DataType::Int32))
                    ),
                ]
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn parse_create_table_with_nullable() {
    let sql = r#"CREATE TABLE table (k UInt8, `a` Nullable(String), `b` Nullable(DateTime64(9, 'UTC')), c Nullable(DateTime64(9)), d Date32 NULL) ENGINE = MergeTree ORDER BY (`k`)"#;
    // ClickHouse has a case-sensitive definition of data type, but canonical representation is not
    let canonical_sql = sql.replace("String", "STRING");

    match clickhouse_and_generic().one_statement_parses_to(sql, &canonical_sql) {
        Statement::CreateTable(CreateTable { name, columns, .. }) => {
            assert_eq!(name, ObjectName::from(vec!["table".into()]));
            assert_eq!(
                columns,
                vec![
                    column_def("k".into(), DataType::UInt8),
                    column_def(
                        Ident::with_quote('`', "a"),
                        DataType::Nullable(Box::new(DataType::String(None)))
                    ),
                    column_def(
                        Ident::with_quote('`', "b"),
                        DataType::Nullable(Box::new(DataType::Datetime64(
                            9,
                            Some("UTC".to_string())
                        )))
                    ),
                    column_def(
                        "c".into(),
                        DataType::Nullable(Box::new(DataType::Datetime64(9, None)))
                    ),
                    ColumnDef {
                        name: "d".into(),
                        data_type: DataType::Date32,
                        options: vec![ColumnOptionDef {
                            name: None,
                            option: ColumnOption::Null
                        }],
                    }
                ]
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn parse_create_table_with_nested_data_types() {
    let sql = concat!(
        "CREATE TABLE table (",
        " i Nested(a Array(Int16), b LowCardinality(String)),",
        " k Array(Tuple(FixedString(128), Int128)),",
        " l Tuple(a DateTime64(9), b Array(UUID)),",
        " m Map(String, UInt16)",
        ") ENGINE=MergeTree ORDER BY (k)"
    );

    match clickhouse().one_statement_parses_to(sql, "") {
        Statement::CreateTable(CreateTable { name, columns, .. }) => {
            assert_eq!(name, ObjectName::from(vec!["table".into()]));
            assert_eq!(
                columns,
                vec![
                    ColumnDef {
                        name: Ident::new("i"),
                        data_type: DataType::Nested(vec![
                            column_def(
                                "a".into(),
                                DataType::Array(ArrayElemTypeDef::Parenthesis(Box::new(
                                    DataType::Int16
                                ),))
                            ),
                            column_def(
                                "b".into(),
                                DataType::LowCardinality(Box::new(DataType::String(None)))
                            )
                        ]),
                        options: vec![],
                    },
                    ColumnDef {
                        name: Ident::new("k"),
                        data_type: DataType::Array(ArrayElemTypeDef::Parenthesis(Box::new(
                            DataType::Tuple(vec![
                                StructField {
                                    field_name: None,
                                    field_type: DataType::FixedString(128),
                                    options: None,
                                },
                                StructField {
                                    field_name: None,
                                    field_type: DataType::Int128,
                                    options: None,
                                }
                            ])
                        ))),
                        options: vec![],
                    },
                    ColumnDef {
                        name: Ident::new("l"),
                        data_type: DataType::Tuple(vec![
                            StructField {
                                field_name: Some("a".into()),
                                field_type: DataType::Datetime64(9, None),
                                options: None,
                            },
                            StructField {
                                field_name: Some("b".into()),
                                field_type: DataType::Array(ArrayElemTypeDef::Parenthesis(
                                    Box::new(DataType::Uuid)
                                )),
                                options: None,
                            },
                        ]),
                        options: vec![],
                    },
                    ColumnDef {
                        name: Ident::new("m"),
                        data_type: DataType::Map(
                            Box::new(DataType::String(None)),
                            Box::new(DataType::UInt16),
                            MapBracketKind::Parentheses
                        ),
                        options: vec![],
                    },
                ]
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn parse_create_table_with_primary_key() {
    match clickhouse_and_generic().verified_stmt(concat!(
        r#"CREATE TABLE db.table (`i` INT, `k` INT)"#,
        " ENGINE = SharedMergeTree('/clickhouse/tables/{uuid}/{shard}', '{replica}')",
        " PRIMARY KEY tuple(i)",
        " ORDER BY tuple(i)",
    )) {
        Statement::CreateTable(CreateTable {
            name,
            columns,
            table_options,
            primary_key,
            order_by,
            ..
        }) => {
            assert_eq!(name.to_string(), "db.table");
            assert_eq!(
                vec![
                    ColumnDef {
                        name: Ident::with_quote('`', "i"),
                        data_type: DataType::Int(None),
                        options: vec![],
                    },
                    ColumnDef {
                        name: Ident::with_quote('`', "k"),
                        data_type: DataType::Int(None),
                        options: vec![],
                    },
                ],
                columns
            );

            let plain_options = match table_options {
                CreateTableOptions::Plain(options) => options,
                _ => unreachable!(),
            };

            assert!(plain_options.contains(&SqlOption::NamedParenthesizedList(
                NamedParenthesizedList {
                    key: Ident::new("ENGINE"),
                    name: Some(Ident::new("SharedMergeTree")),
                    values: vec![
                        Ident::with_quote('\'', "/clickhouse/tables/{uuid}/{shard}"),
                        Ident::with_quote('\'', "{replica}"),
                    ]
                }
            )));

            fn assert_function(actual: &Function, name: &str, arg: &str) -> bool {
                assert_eq!(actual.name, ObjectName::from(vec![Ident::new(name)]));
                assert_eq!(
                    actual.args,
                    FunctionArguments::List(FunctionArgumentList {
                        args: vec![FunctionArg::Unnamed(FunctionArgExpr::Expr(Identifier(
                            Ident::new(arg)
                        )),)],
                        duplicate_treatment: None,
                        clauses: vec![],
                    })
                );
                true
            }
            match primary_key.unwrap().as_ref() {
                Expr::Function(primary_key) => {
                    assert!(assert_function(primary_key, "tuple", "i"));
                }
                _ => panic!("unexpected primary key type"),
            }
            match order_by {
                Some(OneOrManyWithParens::One(Expr::Function(order_by))) => {
                    assert!(assert_function(&order_by, "tuple", "i"));
                }
                _ => panic!("unexpected order by type"),
            };
        }
        _ => unreachable!(),
    }

    clickhouse_and_generic()
        .parse_sql_statements(concat!(
            r#"CREATE TABLE db.table (`i` Int, `k` Int)"#,
            " ORDER BY tuple(i), tuple(k)",
        ))
        .expect_err("ORDER BY supports one expression with tuple");
}

#[test]
fn parse_create_table_with_variant_default_expressions() {
    let sql = concat!(
        "CREATE TABLE table (",
        "a DATETIME MATERIALIZED now(),",
        " b DATETIME EPHEMERAL now(),",
        " c DATETIME EPHEMERAL,",
        " d STRING ALIAS toString(c)",
        ") ENGINE = MergeTree"
    );
    match clickhouse_and_generic().verified_stmt(sql) {
        Statement::CreateTable(CreateTable { columns, .. }) => {
            assert_eq!(
                columns,
                vec![
                    ColumnDef {
                        name: Ident::new("a"),
                        data_type: DataType::Datetime(None),
                        options: vec![ColumnOptionDef {
                            name: None,
                            option: ColumnOption::Materialized(Expr::Function(Function {
                                name: ObjectName::from(vec![Ident::new("now")]),
                                uses_odbc_syntax: false,
                                args: FunctionArguments::List(FunctionArgumentList {
                                    args: vec![],
                                    duplicate_treatment: None,
                                    clauses: vec![],
                                }),
                                parameters: FunctionArguments::None,
                                null_treatment: None,
                                filter: None,
                                over: None,
                                within_group: vec![],
                            }))
                        }],
                    },
                    ColumnDef {
                        name: Ident::new("b"),
                        data_type: DataType::Datetime(None),
                        options: vec![ColumnOptionDef {
                            name: None,
                            option: ColumnOption::Ephemeral(Some(Expr::Function(Function {
                                name: ObjectName::from(vec![Ident::new("now")]),
                                uses_odbc_syntax: false,
                                args: FunctionArguments::List(FunctionArgumentList {
                                    args: vec![],
                                    duplicate_treatment: None,
                                    clauses: vec![],
                                }),
                                parameters: FunctionArguments::None,
                                null_treatment: None,
                                filter: None,
                                over: None,
                                within_group: vec![],
                            })))
                        }],
                    },
                    ColumnDef {
                        name: Ident::new("c"),
                        data_type: DataType::Datetime(None),
                        options: vec![ColumnOptionDef {
                            name: None,
                            option: ColumnOption::Ephemeral(None)
                        }],
                    },
                    ColumnDef {
                        name: Ident::new("d"),
                        data_type: DataType::String(None),
                        options: vec![ColumnOptionDef {
                            name: None,
                            option: ColumnOption::Alias(Expr::Function(Function {
                                name: ObjectName::from(vec![Ident::new("toString")]),
                                uses_odbc_syntax: false,
                                args: FunctionArguments::List(FunctionArgumentList {
                                    args: vec![FunctionArg::Unnamed(FunctionArgExpr::Expr(
                                        Identifier(Ident::new("c"))
                                    ))],
                                    duplicate_treatment: None,
                                    clauses: vec![],
                                }),
                                parameters: FunctionArguments::None,
                                null_treatment: None,
                                filter: None,
                                over: None,
                                within_group: vec![],
                            }))
                        }],
                    }
                ]
            )
        }
        _ => unreachable!(),
    }
}

#[test]
fn parse_create_view_with_fields_data_types() {
    match clickhouse().verified_stmt(r#"CREATE VIEW v (i "int", f "String") AS SELECT * FROM t"#) {
        Statement::CreateView(CreateView { name, columns, .. }) => {
            assert_eq!(name, ObjectName::from(vec!["v".into()]));
            assert_eq!(
                columns,
                vec![
                    ViewColumnDef {
                        name: "i".into(),
                        data_type: Some(DataType::Custom(
                            ObjectName::from(vec![Ident {
                                value: "int".into(),
                                quote_style: Some('"'),
                                span: Span::empty(),
                            }]),
                            vec![]
                        )),
                        options: None,
                    },
                    ViewColumnDef {
                        name: "f".into(),
                        data_type: Some(DataType::Custom(
                            ObjectName::from(vec![Ident {
                                value: "String".into(),
                                quote_style: Some('"'),
                                span: Span::empty(),
                            }]),
                            vec![]
                        )),
                        options: None,
                    },
                ]
            );
        }
        _ => unreachable!(),
    }

    clickhouse()
        .parse_sql_statements(r#"CREATE VIEW v (i, f) AS SELECT * FROM t"#)
        .expect_err("CREATE VIEW with fields and without data types should be invalid");
}

#[test]
fn parse_double_equal() {
    clickhouse().one_statement_parses_to(
        r#"SELECT foo FROM bar WHERE buz == 'buz'"#,
        r#"SELECT foo FROM bar WHERE buz = 'buz'"#,
    );
}

#[test]
fn parse_limit_by() {
    clickhouse_and_generic().verified_stmt(
        r#"SELECT * FROM default.last_asset_runs_mv ORDER BY created_at DESC LIMIT 1 BY asset"#,
    );
    clickhouse_and_generic().verified_stmt(
        r#"SELECT * FROM default.last_asset_runs_mv ORDER BY created_at DESC LIMIT 1 BY asset, toStartOfDay(created_at)"#,
    );
    clickhouse_and_generic().parse_sql_statements(
        r#"SELECT * FROM default.last_asset_runs_mv ORDER BY created_at DESC BY asset, toStartOfDay(created_at)"#,
    ).expect_err("BY without LIMIT");
    clickhouse_and_generic()
        .parse_sql_statements("SELECT * FROM T OFFSET 5 BY foo")
        .expect_err("BY with OFFSET but without LIMIT");
}

#[test]
fn parse_limit_by_with_offset() {
    // `BY` comes last, after either spelling of the offset. ClickHouse reads
    // both of these as `LIMIT 1, 2 BY a`.
    clickhouse_and_generic().verified_stmt("SELECT a FROM t LIMIT 1, 2 BY a");
    clickhouse_and_generic().verified_stmt("SELECT a FROM t LIMIT 2 OFFSET 1 BY a");
    clickhouse_and_generic().verified_stmt("SELECT a FROM t LIMIT 1, 2 BY a, b");

    match clickhouse_and_generic()
        .verified_query("SELECT a FROM t LIMIT 1, 2 BY a")
        .limit_clause
    {
        Some(LimitClause::OffsetCommaLimit {
            offset,
            limit,
            limit_by,
        }) => {
            assert_eq!(offset.to_string(), "1");
            assert_eq!(limit.to_string(), "2");
            assert_eq!(limit_by.len(), 1);
            assert_eq!(limit_by[0].to_string(), "a");
        }
        other => panic!("expected LIMIT <offset>, <limit> BY, got {other:?}"),
    }

    match clickhouse_and_generic()
        .verified_query("SELECT a FROM t LIMIT 2 OFFSET 1 BY a")
        .limit_clause
    {
        Some(LimitClause::LimitOffset {
            limit,
            offset,
            limit_by,
        }) => {
            assert_eq!(limit.map(|l| l.to_string()).as_deref(), Some("2"));
            assert_eq!(offset.map(|o| o.to_string()).as_deref(), Some("OFFSET 1"));
            assert_eq!(limit_by[0].to_string(), "a");
        }
        other => panic!("expected LIMIT ... OFFSET ... BY, got {other:?}"),
    }

    // ClickHouse requires `BY` after the offset. This has always accepted the
    // other order as well, and renders it back in the accepted one.
    clickhouse_and_generic().one_statement_parses_to(
        "SELECT a FROM t LIMIT 2 BY a OFFSET 1",
        "SELECT a FROM t LIMIT 2 OFFSET 1 BY a",
    );
}

#[test]
fn parse_settings_in_query() {
    fn check_settings(sql: &str, expected: Vec<Setting>) {
        match clickhouse_and_generic().verified_stmt(sql) {
            Statement::Query(q) => {
                assert_eq!(q.settings, Some(expected));
            }
            _ => unreachable!(),
        }
    }

    for (sql, expected_settings) in [
        (
            r#"SELECT * FROM t SETTINGS max_threads = 1, max_block_size = 10000"#,
            vec![
                Setting {
                    key: Ident::new("max_threads"),
                    value: Expr::value(number("1")),
                },
                Setting {
                    key: Ident::new("max_block_size"),
                    value: Expr::value(number("10000")),
                },
            ],
        ),
        (
            r#"SELECT * FROM t SETTINGS additional_table_filters = {'table_1': 'x != 2'}"#,
            vec![Setting {
                key: Ident::new("additional_table_filters"),
                value: Expr::Dictionary(vec![DictionaryField {
                    key: Ident::with_quote('\'', "table_1"),
                    value: Expr::value(single_quoted_string("x != 2")).into(),
                }]),
            }],
        ),
        (
            r#"SELECT * FROM t SETTINGS additional_result_filter = 'x != 2', query_plan_optimize_lazy_materialization = false"#,
            vec![
                Setting {
                    key: Ident::new("additional_result_filter"),
                    value: Expr::value(single_quoted_string("x != 2")),
                },
                Setting {
                    key: Ident::new("query_plan_optimize_lazy_materialization"),
                    value: Expr::value(Boolean(false)),
                },
            ],
        ),
    ] {
        check_settings(sql, expected_settings);
    }

    let invalid_cases = vec![
        ("SELECT * FROM t SETTINGS a", "Expected: =, found: EOF"),
        (
            "SELECT * FROM t SETTINGS a=",
            "Expected: an expression, found: EOF",
        ),
        ("SELECT * FROM t SETTINGS a=1, b", "Expected: =, found: EOF"),
        (
            "SELECT * FROM t SETTINGS a=1, b=",
            "Expected: an expression, found: EOF",
        ),
        (
            "SELECT * FROM t SETTINGS a = {",
            "Expected: identifier, found: EOF",
        ),
        (
            "SELECT * FROM t SETTINGS a = {'b'",
            "Expected: :, found: EOF",
        ),
        (
            "SELECT * FROM t SETTINGS a = {'b': ",
            "Expected: an expression, found: EOF",
        ),
        (
            "SELECT * FROM t SETTINGS a = {'b': 'c',}",
            "Expected: identifier, found: }",
        ),
        (
            "SELECT * FROM t SETTINGS a = {'b': 'c', 'd'}",
            "Expected: :, found: }",
        ),
        (
            "SELECT * FROM t SETTINGS a = {'b': 'c', 'd': }",
            "Expected: an expression, found: }",
        ),
        (
            "SELECT * FROM t SETTINGS a = {ANY(b)}",
            "Expected: :, found: (",
        ),
    ];
    for (sql, error_msg) in invalid_cases {
        assert_eq!(
            clickhouse_and_generic()
                .parse_sql_statements(sql)
                .unwrap_err(),
            ParserError(error_msg.to_string())
        );
    }
}
#[test]
fn parse_select_star_except() {
    clickhouse().verified_stmt("SELECT * EXCEPT (prev_status) FROM anomalies");
}

#[test]
fn parse_select_parametric_function() {
    match clickhouse_and_generic().verified_stmt("SELECT HISTOGRAM(0.5, 0.6)(x, y) FROM t") {
        Statement::Query(query) => {
            let projection: &Vec<SelectItem> = query.body.as_select().unwrap().projection.as_ref();
            assert_eq!(projection.len(), 1);
            match &projection[0] {
                UnnamedExpr(Expr::Function(f)) => {
                    let args = match &f.args {
                        FunctionArguments::List(ref args) => args,
                        _ => unreachable!(),
                    };
                    assert_eq!(args.args.len(), 2);
                    assert_eq!(
                        args.args[0],
                        FunctionArg::Unnamed(FunctionArgExpr::Expr(Identifier(Ident::from("x"))))
                    );
                    assert_eq!(
                        args.args[1],
                        FunctionArg::Unnamed(FunctionArgExpr::Expr(Identifier(Ident::from("y"))))
                    );

                    let parameters = match f.parameters {
                        FunctionArguments::List(ref args) => args,
                        _ => unreachable!(),
                    };
                    assert_eq!(parameters.args.len(), 2);
                    assert_eq!(
                        parameters.args[0],
                        FunctionArg::Unnamed(FunctionArgExpr::Expr(Expr::Value(
                            (Value::Number("0.5".parse().unwrap(), false)).with_empty_span()
                        )))
                    );
                    assert_eq!(
                        parameters.args[1],
                        FunctionArg::Unnamed(FunctionArgExpr::Expr(Expr::Value(
                            (Value::Number("0.6".parse().unwrap(), false)).with_empty_span()
                        )))
                    );
                }
                _ => unreachable!(),
            }
        }
        _ => unreachable!(),
    }
}

#[test]
fn parse_select_star_except_no_parens() {
    clickhouse().one_statement_parses_to(
        "SELECT * EXCEPT prev_status FROM anomalies",
        "SELECT * EXCEPT (prev_status) FROM anomalies",
    );
}

#[test]
fn parse_create_materialized_view() {
    // example sql
    // https://clickhouse.com/docs/en/guides/developer/cascading-materialized-views
    let sql = concat!(
        "CREATE MATERIALIZED VIEW analytics.monthly_aggregated_data_mv ",
        "TO analytics.monthly_aggregated_data ",
        "AS SELECT toDate(toStartOfMonth(event_time)) ",
        "AS month, domain_name, sumState(count_views) ",
        "AS sumCountViews FROM analytics.hourly_data ",
        "GROUP BY domain_name, month"
    );
    clickhouse_and_generic().verified_stmt(sql);
}

#[test]
fn parse_select_order_by_with_fill_interpolate() {
    let sql = "SELECT id, fname, lname FROM customer WHERE id < 5 \
        ORDER BY \
            fname ASC NULLS FIRST WITH FILL FROM 10 TO 20 STEP 2, \
            lname DESC NULLS LAST WITH FILL FROM 30 TO 40 STEP 3 \
            INTERPOLATE (col1 AS col1 + 1) \
        LIMIT 2";
    let select = clickhouse().verified_query(sql);
    assert_eq!(
        OrderBy {
            kind: OrderByKind::Expressions(vec![
                OrderByExpr {
                    expr: Expr::Identifier(Ident::new("fname")),
                    options: OrderByOptions {
                        sort: Some(OrderBySort::Asc),
                        nulls_first: Some(true),
                    },
                    with_fill: Some(WithFill {
                        from: Some(Expr::value(number("10"))),
                        to: Some(Expr::value(number("20"))),
                        step: Some(Expr::value(number("2"))),
                    }),
                },
                OrderByExpr {
                    expr: Expr::Identifier(Ident::new("lname")),
                    options: OrderByOptions {
                        sort: Some(OrderBySort::Desc),
                        nulls_first: Some(false),
                    },
                    with_fill: Some(WithFill {
                        from: Some(Expr::value(number("30"))),
                        to: Some(Expr::value(number("40"))),
                        step: Some(Expr::value(number("3"))),
                    }),
                },
            ]),
            interpolate: Some(Interpolate {
                exprs: Some(vec![InterpolateExpr {
                    column: Ident::new("col1"),
                    expr: Some(Expr::BinaryOp {
                        left: Box::new(Expr::Identifier(Ident::new("col1"))),
                        op: BinaryOperator::Plus,
                        right: Box::new(Expr::value(number("1"))),
                    }),
                }])
            })
        },
        select.order_by.expect("ORDER BY expected")
    );
    assert_eq!(
        select.limit_clause,
        Some(LimitClause::LimitOffset {
            limit: Some(Expr::value(number("2"))),
            offset: None,
            limit_by: vec![]
        })
    );
}

#[test]
fn parse_select_order_by_with_fill_interpolate_multi_interpolates() {
    let sql = "SELECT id, fname, lname FROM customer ORDER BY fname WITH FILL \
        INTERPOLATE (col1 AS col1 + 1) INTERPOLATE (col2 AS col2 + 2)";
    clickhouse_and_generic()
        .parse_sql_statements(sql)
        .expect_err("ORDER BY only accepts a single INTERPOLATE clause");
}

#[test]
fn parse_select_order_by_with_fill_interpolate_multi_with_fill_interpolates() {
    let sql = "SELECT id, fname, lname FROM customer \
        ORDER BY \
            fname WITH FILL INTERPOLATE (col1 AS col1 + 1), \
            lname WITH FILL INTERPOLATE (col2 AS col2 + 2)";
    clickhouse_and_generic()
        .parse_sql_statements(sql)
        .expect_err("ORDER BY only accepts a single INTERPOLATE clause");
}

#[test]
fn parse_select_order_by_interpolate_not_last() {
    let sql = "SELECT id, fname, lname FROM customer \
        ORDER BY \
            fname INTERPOLATE (col2 AS col2 + 2),
            lname";
    clickhouse_and_generic()
        .parse_sql_statements(sql)
        .expect_err("ORDER BY INTERPOLATE must be in the last position");
}

#[test]
fn parse_with_fill() {
    let sql = "SELECT fname FROM customer ORDER BY fname \
        WITH FILL FROM 10 TO 20 STEP 2";
    let select = clickhouse().verified_query(sql);
    assert_eq!(
        Some(WithFill {
            from: Some(Expr::value(number("10"))),
            to: Some(Expr::value(number("20"))),
            step: Some(Expr::value(number("2"))),
        })
        .as_ref(),
        match select.order_by.expect("ORDER BY expected").kind {
            OrderByKind::Expressions(ref exprs) => exprs[0].with_fill.as_ref(),
            _ => None,
        }
    );
}

#[test]
fn parse_with_fill_missing_single_argument() {
    let sql = "SELECT id, fname, lname FROM customer ORDER BY \
            fname WITH FILL FROM TO 20";
    clickhouse_and_generic()
        .parse_sql_statements(sql)
        .expect_err("WITH FILL requires expressions for all arguments");
}

#[test]
fn parse_with_fill_multiple_incomplete_arguments() {
    let sql = "SELECT id, fname, lname FROM customer ORDER BY \
            fname WITH FILL FROM TO 20, lname WITH FILL FROM TO STEP 1";
    clickhouse_and_generic()
        .parse_sql_statements(sql)
        .expect_err("WITH FILL requires expressions for all arguments");
}

#[test]
fn parse_interpolate_body_with_columns() {
    let sql = "SELECT fname FROM customer ORDER BY fname WITH FILL \
        INTERPOLATE (col1 AS col1 + 1, col2 AS col3, col4 AS col4 + 4)";
    let select = clickhouse().verified_query(sql);
    assert_eq!(
        Some(Interpolate {
            exprs: Some(vec![
                InterpolateExpr {
                    column: Ident::new("col1"),
                    expr: Some(Expr::BinaryOp {
                        left: Box::new(Expr::Identifier(Ident::new("col1"))),
                        op: BinaryOperator::Plus,
                        right: Box::new(Expr::value(number("1"))),
                    }),
                },
                InterpolateExpr {
                    column: Ident::new("col2"),
                    expr: Some(Expr::Identifier(Ident::new("col3"))),
                },
                InterpolateExpr {
                    column: Ident::new("col4"),
                    expr: Some(Expr::BinaryOp {
                        left: Box::new(Expr::Identifier(Ident::new("col4"))),
                        op: BinaryOperator::Plus,
                        right: Box::new(Expr::value(number("4"))),
                    }),
                },
            ])
        })
        .as_ref(),
        select
            .order_by
            .expect("ORDER BY expected")
            .interpolate
            .as_ref()
    );
}

#[test]
fn parse_interpolate_without_body() {
    let sql = "SELECT fname FROM customer ORDER BY fname WITH FILL INTERPOLATE";
    let select = clickhouse().verified_query(sql);
    assert_eq!(
        Some(Interpolate { exprs: None }).as_ref(),
        select
            .order_by
            .expect("ORDER BY expected")
            .interpolate
            .as_ref()
    );
}

#[test]
fn parse_interpolate_with_empty_body() {
    let sql = "SELECT fname FROM customer ORDER BY fname WITH FILL INTERPOLATE ()";
    let select = clickhouse().verified_query(sql);
    assert_eq!(
        Some(Interpolate {
            exprs: Some(vec![])
        })
        .as_ref(),
        select
            .order_by
            .expect("ORDER BY expected")
            .interpolate
            .as_ref()
    );
}

#[test]
fn test_prewhere() {
    match clickhouse_and_generic().verified_stmt("SELECT * FROM t PREWHERE x = 1 WHERE y = 2") {
        Statement::Query(query) => {
            let prewhere = query.body.as_select().unwrap().prewhere.as_ref();
            assert_eq!(
                prewhere,
                Some(&BinaryOp {
                    left: Box::new(Identifier(Ident::new("x"))),
                    op: BinaryOperator::Eq,
                    right: Box::new(Expr::Value(
                        (Value::Number("1".parse().unwrap(), false)).with_empty_span()
                    )),
                })
            );
            let selection = query.as_ref().body.as_select().unwrap().selection.as_ref();
            assert_eq!(
                selection,
                Some(&BinaryOp {
                    left: Box::new(Identifier(Ident::new("y"))),
                    op: BinaryOperator::Eq,
                    right: Box::new(Expr::Value(
                        (Value::Number("2".parse().unwrap(), false)).with_empty_span()
                    )),
                })
            );
        }
        _ => unreachable!(),
    }

    match clickhouse_and_generic().verified_stmt("SELECT * FROM t PREWHERE x = 1 AND y = 2") {
        Statement::Query(query) => {
            let prewhere = query.body.as_select().unwrap().prewhere.as_ref();
            assert_eq!(
                prewhere,
                Some(&BinaryOp {
                    left: Box::new(BinaryOp {
                        left: Box::new(Identifier(Ident::new("x"))),
                        op: BinaryOperator::Eq,
                        right: Box::new(Expr::Value(
                            (Value::Number("1".parse().unwrap(), false)).with_empty_span()
                        )),
                    }),
                    op: BinaryOperator::And,
                    right: Box::new(BinaryOp {
                        left: Box::new(Identifier(Ident::new("y"))),
                        op: BinaryOperator::Eq,
                        right: Box::new(Expr::Value(
                            (Value::Number("2".parse().unwrap(), false)).with_empty_span()
                        )),
                    }),
                })
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn parse_use() {
    let valid_object_names = [
        "mydb",
        "SCHEMA",
        "DATABASE",
        "CATALOG",
        "WAREHOUSE",
        "DEFAULT",
    ];
    let quote_styles = ['"', '`'];

    for object_name in &valid_object_names {
        // Test single identifier without quotes
        assert_eq!(
            clickhouse().verified_stmt(&format!("USE {object_name}")),
            Statement::Use(Use::Object(ObjectName::from(vec![Ident::new(
                object_name.to_string()
            )])))
        );
        for &quote in &quote_styles {
            // Test single identifier with different type of quotes
            assert_eq!(
                clickhouse().verified_stmt(&format!("USE {quote}{object_name}{quote}")),
                Statement::Use(Use::Object(ObjectName::from(vec![Ident::with_quote(
                    quote,
                    object_name.to_string(),
                )])))
            );
        }
    }
}

#[test]
fn test_query_with_format_clause() {
    let format_options = vec!["TabSeparated", "JSONCompact", "NULL"];
    for format in &format_options {
        let sql = format!("SELECT * FROM t FORMAT {format}");
        match clickhouse_and_generic().verified_stmt(&sql) {
            Statement::Query(query) => {
                if *format == "NULL" {
                    assert_eq!(query.format_clause, Some(FormatClause::Null));
                } else {
                    assert_eq!(
                        query.format_clause,
                        Some(FormatClause::Identifier(Ident::new(*format)))
                    );
                }
            }
            _ => unreachable!(),
        }
    }

    let invalid_cases = [
        "SELECT * FROM t FORMAT",
        "SELECT * FROM t FORMAT TabSeparated JSONCompact",
        "SELECT * FROM t FORMAT TabSeparated TabSeparated",
    ];
    for sql in &invalid_cases {
        clickhouse_and_generic()
            .parse_sql_statements(sql)
            .expect_err("Expected: FORMAT {identifier}, found: ");
    }
}

#[test]
fn test_insert_query_with_format_clause() {
    let cases = [
        r#"INSERT INTO tbl FORMAT JSONEachRow {"id": 1, "value": "foo"}, {"id": 2, "value": "bar"}"#,
        r#"INSERT INTO tbl FORMAT JSONEachRow ["first", "second", "third"]"#,
        r#"INSERT INTO tbl FORMAT JSONEachRow [{"first": 1}]"#,
        r#"INSERT INTO tbl (foo) FORMAT JSONAsObject {"foo": {"bar": {"x": "y"}, "baz": 1}}"#,
        r#"INSERT INTO tbl (foo, bar) FORMAT JSON {"foo": 1, "bar": 2}"#,
        r#"INSERT INTO tbl FORMAT CSV col1, col2, col3"#,
        r#"INSERT INTO tbl FORMAT LineAsString "I love apple", "I love banana", "I love orange""#,
        r#"INSERT INTO tbl (foo) SETTINGS input_format_json_read_bools_as_numbers = true FORMAT JSONEachRow {"id": 1, "value": "foo"}"#,
        r#"INSERT INTO tbl SETTINGS format_template_resultset = '/some/path/resultset.format', format_template_row = '/some/path/row.format' FORMAT Template"#,
        r#"INSERT INTO tbl SETTINGS input_format_json_read_bools_as_numbers = true FORMAT JSONEachRow {"id": 1, "value": "foo"}"#,
    ];

    for sql in &cases {
        clickhouse().verified_stmt(sql);
    }
}

#[test]
fn parse_create_table_on_commit_and_as_query() {
    let sql = r#"CREATE LOCAL TEMPORARY TABLE test ON COMMIT PRESERVE ROWS AS SELECT 1"#;
    match clickhouse_and_generic().verified_stmt(sql) {
        Statement::CreateTable(CreateTable {
            name,
            on_commit,
            query,
            ..
        }) => {
            assert_eq!(name.to_string(), "test");
            assert_eq!(on_commit, Some(OnCommit::PreserveRows));
            assert_eq!(
                query.unwrap().body.as_select().unwrap().projection,
                vec![UnnamedExpr(Expr::Value(
                    (Value::Number("1".parse().unwrap(), false)).with_empty_span()
                ))]
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn parse_freeze_and_unfreeze_partition() {
    // test cases without `WITH NAME`
    for operation_name in &["FREEZE", "UNFREEZE"] {
        let sql = format!("ALTER TABLE t {operation_name} PARTITION '2024-08-14'");

        let expected_partition = Partition::Expr(Expr::Value(
            Value::SingleQuotedString("2024-08-14".to_string()).with_empty_span(),
        ));
        match clickhouse_and_generic().verified_stmt(&sql) {
            Statement::AlterTable(AlterTable { operations, .. }) => {
                assert_eq!(operations.len(), 1);
                let expected_operation = if operation_name == &"FREEZE" {
                    AlterTableOperation::FreezePartition {
                        partition: expected_partition,
                        with_name: None,
                    }
                } else {
                    AlterTableOperation::UnfreezePartition {
                        partition: expected_partition,
                        with_name: None,
                    }
                };
                assert_eq!(operations[0], expected_operation);
            }
            _ => unreachable!(),
        }
    }

    // test case with `WITH NAME`
    for operation_name in &["FREEZE", "UNFREEZE"] {
        let sql =
            format!("ALTER TABLE t {operation_name} PARTITION '2024-08-14' WITH NAME 'hello'");
        match clickhouse_and_generic().verified_stmt(&sql) {
            Statement::AlterTable(AlterTable { operations, .. }) => {
                assert_eq!(operations.len(), 1);
                let expected_partition = Partition::Expr(Expr::Value(
                    Value::SingleQuotedString("2024-08-14".to_string()).with_empty_span(),
                ));
                let expected_operation = if operation_name == &"FREEZE" {
                    AlterTableOperation::FreezePartition {
                        partition: expected_partition,
                        with_name: Some(Ident::with_quote('\'', "hello")),
                    }
                } else {
                    AlterTableOperation::UnfreezePartition {
                        partition: expected_partition,
                        with_name: Some(Ident::with_quote('\'', "hello")),
                    }
                };
                assert_eq!(operations[0], expected_operation);
            }
            _ => unreachable!(),
        }
    }

    // negative cases
    for operation_name in &["FREEZE", "UNFREEZE"] {
        assert_eq!(
            clickhouse_and_generic()
                .parse_sql_statements(format!("ALTER TABLE t0 {operation_name} PARTITION").as_str())
                .unwrap_err(),
            ParserError("Expected: an expression, found: EOF".to_string())
        );
        assert_eq!(
            clickhouse_and_generic()
                .parse_sql_statements(
                    format!("ALTER TABLE t0 {operation_name} PARTITION p0 WITH").as_str()
                )
                .unwrap_err(),
            ParserError("Expected: NAME, found: EOF".to_string())
        );
        assert_eq!(
            clickhouse_and_generic()
                .parse_sql_statements(
                    format!("ALTER TABLE t0 {operation_name} PARTITION p0 WITH NAME").as_str()
                )
                .unwrap_err(),
            ParserError("Expected: identifier, found: EOF".to_string())
        );
    }
}

#[test]
fn parse_select_table_function_settings() {
    fn check_settings(sql: &str, expected: &TableFunctionArgs) {
        match clickhouse_and_generic().verified_stmt(sql) {
            Statement::Query(q) => {
                let from = &q.body.as_select().unwrap().from;
                assert_eq!(from.len(), 1);
                assert_eq!(from[0].joins, vec![]);
                match &from[0].relation {
                    Table { args, .. } => {
                        let args = args.as_ref().unwrap();
                        assert_eq!(args, expected);
                    }
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }
    check_settings(
        "SELECT * FROM table_function(arg, SETTINGS s0 = 3, s1 = 's')",
        &TableFunctionArgs {
            args: vec![FunctionArg::Unnamed(FunctionArgExpr::Expr(
                Expr::Identifier("arg".into()),
            ))],

            settings: Some(vec![
                Setting {
                    key: "s0".into(),
                    value: Expr::value(number("3")),
                },
                Setting {
                    key: "s1".into(),
                    value: Expr::value(single_quoted_string("s")),
                },
            ]),
            subquery: None,
        },
    );
    check_settings(
        r#"SELECT * FROM table_function(arg)"#,
        &TableFunctionArgs {
            args: vec![FunctionArg::Unnamed(FunctionArgExpr::Expr(
                Expr::Identifier("arg".into()),
            ))],
            settings: None,
            subquery: None,
        },
    );
    check_settings(
        "SELECT * FROM table_function(SETTINGS s0 = 3, s1 = 's')",
        &TableFunctionArgs {
            args: vec![],
            settings: Some(vec![
                Setting {
                    key: "s0".into(),
                    value: Expr::value(number("3")),
                },
                Setting {
                    key: "s1".into(),
                    value: Expr::value(single_quoted_string("s")),
                },
            ]),
            subquery: None,
        },
    );
    let invalid_cases = vec![
        "SELECT * FROM t(SETTINGS a)",
        "SELECT * FROM t(SETTINGS a=)",
        "SELECT * FROM t(SETTINGS a=1, b)",
        "SELECT * FROM t(SETTINGS a=1, b=)",
    ];
    for sql in invalid_cases {
        clickhouse_and_generic()
            .parse_sql_statements(sql)
            .expect_err("Expected: SETTINGS key = value, found: ");
    }
}

#[test]
fn explain_describe() {
    clickhouse().verified_stmt("DESCRIBE test.table");
    clickhouse().verified_stmt("DESCRIBE TABLE test.table");
}

#[test]
fn explain_desc() {
    clickhouse().verified_stmt("DESC test.table");
    clickhouse().verified_stmt("DESC TABLE test.table");
}

#[test]
fn parse_explain_table() {
    match clickhouse().verified_stmt("EXPLAIN TABLE test_identifier") {
        Statement::ExplainTable {
            describe_alias,
            hive_format,
            has_table_keyword,
            table_name,
        } => {
            pretty_assertions::assert_eq!(describe_alias, DescribeAlias::Explain);
            pretty_assertions::assert_eq!(hive_format, None);
            pretty_assertions::assert_eq!(has_table_keyword, true);
            pretty_assertions::assert_eq!("test_identifier", table_name.to_string());
        }
        _ => panic!("Unexpected Statement, must be ExplainTable"),
    }
}

#[test]
fn parse_table_sample() {
    clickhouse().verified_stmt("SELECT * FROM tbl SAMPLE 0.1");
    clickhouse().verified_stmt("SELECT * FROM tbl SAMPLE 1000");
    clickhouse().verified_stmt("SELECT * FROM tbl SAMPLE 1 / 10");
    clickhouse().verified_stmt("SELECT * FROM tbl SAMPLE 1 / 10 OFFSET 1 / 2");
}

#[test]
fn test_parse_not_null_in_column_options() {
    // In addition to DEFAULT and CHECK ClickHouse also supports MATERIALIZED, all of which
    // can contain `IS NOT NULL` and thus `NOT NULL` as an alias.
    let canonical = concat!(
        "CREATE TABLE foo (",
        "abc INT DEFAULT (42 IS NOT NULL) NOT NULL,",
        " not_null BOOL MATERIALIZED (abc IS NOT NULL),",
        " CHECK (abc IS NOT NULL)",
        ")",
    );
    clickhouse().verified_stmt(canonical);
    clickhouse().one_statement_parses_to(
        concat!(
            "CREATE TABLE foo (",
            "abc INT DEFAULT (42 NOT NULL) NOT NULL,",
            " not_null BOOL MATERIALIZED (abc NOT NULL),",
            " CHECK (abc NOT NULL)",
            ")",
        ),
        canonical,
    );
}

/// Unwraps the single `ARRAY JOIN` clause of a query.
fn array_join_of(dialects: TestedDialects, sql: &str) -> ArrayJoin {
    match dialects.verified_stmt(sql) {
        Statement::Query(query) => {
            let from = &query.body.as_select().unwrap().from[0];
            assert_eq!(from.array_joins.len(), 1);
            from.array_joins[0].clone()
        }
        _ => unreachable!(),
    }
}

#[test]
fn parse_array_join() {
    let array_join = array_join_of(
        clickhouse_and_generic(),
        "SELECT x FROM t ARRAY JOIN arr AS x",
    );
    assert_eq!(array_join.kind, ArrayJoinKind::Default);
    assert_eq!(
        array_join.exprs,
        vec![ExprWithAlias {
            expr: Expr::Identifier(Ident::new("arr")),
            alias: Some(Ident::new("x")),
        }]
    );

    // The alias is optional.
    let array_join = array_join_of(clickhouse_and_generic(), "SELECT arr FROM t ARRAY JOIN arr");
    assert_eq!(
        array_join.exprs,
        vec![ExprWithAlias {
            expr: Expr::Identifier(Ident::new("arr")),
            alias: None,
        }]
    );

    // Regular joins come first, then the ARRAY JOIN.
    clickhouse_and_generic()
        .verified_stmt("SELECT x FROM t JOIN u ON t.id = u.id ARRAY JOIN arr AS x");

    // ARRAY JOIN requires at least one expression.
    clickhouse_and_generic()
        .parse_sql_statements("SELECT x FROM t ARRAY JOIN")
        .expect_err("ARRAY JOIN requires an array expression");
}

#[test]
fn parse_array_join_multiple_expressions() {
    // The operands are a comma-separated list of arbitrary expressions, not
    // relations. Checked on ClickHouse only: GenericDialect has no lambdas and
    // parses `x -> x + 1` as a `->` binary operator.
    // relations: array columns, function calls, lambdas and array literals.
    let sql = concat!(
        "SELECT s, num, mapped FROM t ",
        "ARRAY JOIN arr AS a, arrayEnumerate(arr) AS num, arrayMap(x -> x + 1, arr) AS mapped"
    );
    let array_join = array_join_of(clickhouse(), sql);
    assert_eq!(array_join.kind, ArrayJoinKind::Default);
    assert_eq!(
        array_join
            .exprs
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>(),
        vec![
            "arr AS a",
            "arrayEnumerate(arr) AS num",
            "arrayMap(x -> x + 1, arr) AS mapped",
        ]
    );

    clickhouse_and_generic().verified_stmt("SELECT a FROM t ARRAY JOIN [1, 2, 3] AS a");
}

#[test]
fn parse_left_array_join() {
    // LEFT ARRAY JOIN preserves rows with empty arrays.
    let array_join = array_join_of(
        clickhouse_and_generic(),
        "SELECT x FROM t LEFT ARRAY JOIN arr AS x",
    );
    assert_eq!(array_join.kind, ArrayJoinKind::Left);
}

#[test]
fn parse_inner_array_join() {
    // INNER ARRAY JOIN is an explicit spelling of the default behavior.
    let array_join = array_join_of(
        clickhouse_and_generic(),
        "SELECT x FROM t INNER ARRAY JOIN arr AS x",
    );
    assert_eq!(array_join.kind, ArrayJoinKind::Inner);
}

#[test]
#[cfg(feature = "visitor")]
fn visit_array_join_operand_is_not_a_relation() {
    // An array column is not a table: walking relations must yield only `t`.
    let stmt = clickhouse().verified_stmt("SELECT x FROM t ARRAY JOIN arr AS x");
    let mut relations = vec![];
    let _ = sqlparser::ast::visit_relations(&stmt, |name| {
        relations.push(name.to_string());
        core::ops::ControlFlow::<()>::Continue(())
    });
    assert_eq!(relations, ["t"]);
}

#[test]
fn parse_in_unparenthesized_expr() {
    // IN [expr] parses to IN ([expr]) and does not cause regressions
    clickhouse().expr_parses_to("x IN 'a'", "x IN ('a')");

    // The branch must not fire when the next token is `(` (regressions).
    clickhouse().verified_expr("x IN (1, 2, 3)");
    clickhouse().verified_stmt("SELECT * FROM t WHERE x IN (SELECT y FROM u)");
}

#[test]
fn parse_in_table() {
    // A bare name on the right-hand side of `IN` is a table, not a value:
    // `UserID IN users` reads the whole table. It must round-trip without gaining
    // parentheses, and the table must be reachable as a relation.
    let select = clickhouse().verified_only_select("SELECT * FROM t WHERE a IN users");
    assert_eq!(
        Expr::InTable {
            expr: Box::new(Expr::Identifier(Ident::new("a"))),
            table: ObjectName::from(vec![Ident::new("users")]),
            negated: false,
        },
        select.selection.unwrap()
    );

    let select = clickhouse().verified_only_select("SELECT * FROM t WHERE a NOT IN dim.users");
    assert_eq!(
        Expr::InTable {
            expr: Box::new(Expr::Identifier(Ident::new("a"))),
            table: ObjectName::from(vec![Ident::new("dim"), Ident::new("users")]),
            negated: true,
        },
        select.selection.unwrap()
    );

    // Everything else on the right-hand side stays an ordinary list.
    clickhouse().expr_parses_to("x IN 'a'", "x IN ('a')");
    clickhouse().expr_parses_to("x IN 1", "x IN (1)");
    clickhouse().verified_expr("x IN (1, 2, 3)");
    clickhouse().verified_stmt("SELECT * FROM t WHERE x IN (SELECT y FROM u)");
}

#[test]
#[cfg(feature = "visitor")]
fn visit_in_table_relation() {
    // The point of the dedicated variant: the table read by `IN <table>` is
    // reachable by consumers that walk relations, such as lineage extraction.
    let stmt = clickhouse().verified_stmt("SELECT * FROM t WHERE a IN dim.users");
    let mut relations = vec![];
    let _ = sqlparser::ast::visit_relations(&stmt, |name| {
        relations.push(name.to_string());
        core::ops::ControlFlow::<()>::Continue(())
    });
    assert_eq!(relations, ["t", "dim.users"]);
}

#[test]
fn parse_in_unparenthesized_dictionary_placeholder() {
    // IN [{placeholder:Type}] parses to IN ({placholder:Type})
    clickhouse().expr_parses_to("x IN {ids:Array(UInt64)}", "x IN ({ids: Array(UInt64)})");
    clickhouse().expr_parses_to(
        "x NOT IN {ids:Array(UInt64)}",
        "x NOT IN ({ids: Array(UInt64)})",
    );
    clickhouse().verified_expr("x IN ({ids: Array(UInt64)})");
    // Precedence: the trailing `AND` is not swallowed.
    clickhouse().verified_expr("x IN ({p: Array(UInt64)}) AND y = 1");
}

#[test]
fn parse_asof_join() {
    // ASOF JOIN with an ON constraint, where the closest-match (inequality) condition is
    // the last conjunct. ClickHouse does not use Snowflake's MATCH_CONDITION clause.
    let sql = concat!(
        "SELECT t.symbol, q.bid FROM md.trades AS t ",
        "ASOF JOIN md.quotes AS q ON t.symbol = q.symbol AND t.ts >= q.ts"
    );
    match clickhouse_and_generic().verified_stmt(sql) {
        Statement::Query(query) => {
            let select = query.body.as_select().unwrap();
            let join = &select.from[0].joins[0];
            assert!(matches!(
                join.join_operator,
                JoinOperator::AsOfJoin(JoinConstraint::On(_))
            ));
        }
        _ => unreachable!(),
    }

    // ASOF JOIN with a USING constraint, where the ASOF column is listed last.
    match clickhouse_and_generic()
        .verified_stmt("SELECT * FROM trades AS t ASOF JOIN quotes AS q USING(symbol, ts)")
    {
        Statement::Query(query) => {
            let select = query.body.as_select().unwrap();
            let join = &select.from[0].joins[0];
            assert!(matches!(
                join.join_operator,
                JoinOperator::AsOfJoin(JoinConstraint::Using(_))
            ));
        }
        _ => unreachable!(),
    }

    // As for the other join operators, a missing constraint is left to the analyzer.
    clickhouse_and_generic().verified_stmt("SELECT * FROM trades AS t ASOF JOIN quotes AS q");
}

#[test]
fn parse_asof_left_join() {
    let sql = concat!(
        "SELECT t.symbol, q.bid FROM md.trades AS t ",
        "ASOF LEFT JOIN md.quotes AS q ON t.symbol = q.symbol AND t.ts >= q.ts"
    );
    match clickhouse_and_generic().verified_stmt(sql) {
        Statement::Query(query) => {
            let select = query.body.as_select().unwrap();
            let join = &select.from[0].joins[0];
            assert!(matches!(
                join.join_operator,
                JoinOperator::AsOfLeftJoin(JoinConstraint::On(_))
            ));
        }
        _ => unreachable!(),
    }

    clickhouse_and_generic()
        .verified_stmt("SELECT * FROM trades AS t ASOF LEFT JOIN quotes AS q USING(symbol, ts)");

    // ClickHouse documents the join kind before the strictness modifier, so
    // `LEFT ASOF JOIN` is accepted as well. `ASOF LEFT JOIN` is the canonical form.
    clickhouse_and_generic().one_statement_parses_to(
        "SELECT * FROM trades AS t LEFT ASOF JOIN quotes AS q USING (ts)",
        "SELECT * FROM trades AS t ASOF LEFT JOIN quotes AS q USING(ts)",
    );

    // Only `LEFT` pairs with `ASOF`; ClickHouse has no right/full ASOF join.
    clickhouse_and_generic()
        .parse_sql_statements("SELECT * FROM trades AS t RIGHT ASOF JOIN quotes AS q USING (ts)")
        .expect_err("RIGHT ASOF JOIN is not valid ClickHouse syntax");
}

#[test]
fn parse_asof_left_join_rejects_match_condition() {
    // Snowflake's `MATCH_CONDITION` flavor of `ASOF JOIN` has no left/inner
    // distinction, so combining it with `LEFT` would silently drop the `LEFT` and
    // turn an outer join into an inner one.
    let sql = concat!(
        "SELECT * FROM trades AS t ASOF LEFT JOIN quotes AS q ",
        "MATCH_CONDITION (t.ts >= q.ts) ON t.sym = q.sym"
    );
    assert_eq!(
        ParserError(
            "Expected: ON or USING after ASOF LEFT JOIN, found: MATCH_CONDITION".to_string()
        ),
        clickhouse_and_generic()
            .parse_sql_statements(sql)
            .unwrap_err()
    );
}

/// Unwraps the `WITH` list of a query, asserting it has `len` entries.
fn with_items(query: &Query, len: usize) -> &[WithItem] {
    let with = query.with.as_ref().expect("expected a WITH clause");
    assert_eq!(with.cte_tables.len(), len);
    &with.cte_tables
}

fn expect_scalar(cte_table: &WithItem) -> &ScalarWithItem {
    match cte_table {
        WithItem::Scalar(scalar) => scalar,
        other => panic!("Expected: a scalar WITH item, found: {other:?}"),
    }
}

fn expect_cte(cte_table: &WithItem) -> &Cte {
    match cte_table {
        WithItem::Cte(cte) => cte,
        other => panic!("Expected: a standard CTE, found: {other:?}"),
    }
}

// ClickHouse scalar `WITH <expression> AS <identifier>`.
// See <https://clickhouse.com/docs/sql-reference/statements/select/with>.

#[test]
fn parse_scalar_with_subquery() {
    let sql =
        "WITH (SELECT max(ts) FROM raw.watermarks) AS wm SELECT * FROM raw.events WHERE ts > wm";
    let query = clickhouse_and_generic().verified_query(sql);

    let scalar = expect_scalar(&with_items(&query, 1)[0]);
    assert_eq!(scalar.alias, Ident::new("wm"));
    match &scalar.expr {
        Expr::Subquery(subquery) => {
            assert_eq!(subquery.to_string(), "SELECT max(ts) FROM raw.watermarks");
        }
        other => panic!("Expected: a subquery expression, found: {other:?}"),
    }
}

#[test]
fn parse_scalar_with_constant() {
    let sql = "WITH 42 AS magic SELECT magic";
    let query = clickhouse_and_generic().verified_query(sql);

    let scalar = expect_scalar(&with_items(&query, 1)[0]);
    assert_eq!(scalar.alias, Ident::new("magic"));
    assert_eq!(scalar.expr, Expr::value(number("42")));
}

#[test]
fn parse_scalar_with_function_call() {
    // The original report in apache/datafusion-sqlparser-rs#1514.
    let sql = "WITH concat(prefix_addr, '/', prefix_len) AS prefix SELECT prefix FROM tbl";
    let query = clickhouse_and_generic().verified_query(sql);

    let scalar = expect_scalar(&with_items(&query, 1)[0]);
    assert_eq!(scalar.alias, Ident::new("prefix"));
    assert_eq!(
        scalar.expr.to_string(),
        "concat(prefix_addr, '/', prefix_len)"
    );
}

#[test]
fn parse_scalar_with_mixed_with_standard_cte() {
    // ClickHouse allows both flavors in the same comma-separated list, in any order.
    let sql = "WITH 42 AS magic, sub AS (SELECT 1) SELECT magic FROM sub";
    let query = clickhouse_and_generic().verified_query(sql);

    let items = with_items(&query, 2);
    assert_eq!(expect_scalar(&items[0]).alias, Ident::new("magic"));
    assert_eq!(expect_cte(&items[1]).alias.name, Ident::new("sub"));

    // ... and with the standard CTE first.
    let sql = "WITH sub AS (SELECT 1) SELECT magic FROM sub";
    let query = clickhouse_and_generic().verified_query(sql);
    assert_eq!(
        expect_cte(&with_items(&query, 1)[0]).alias.name,
        Ident::new("sub")
    );

    let sql = "WITH sub AS (SELECT 1), 42 AS magic SELECT magic FROM sub";
    let query = clickhouse_and_generic().verified_query(sql);
    let items = with_items(&query, 2);
    assert_eq!(expect_cte(&items[0]).alias.name, Ident::new("sub"));
    assert_eq!(expect_scalar(&items[1]).alias, Ident::new("magic"));
}

#[test]
fn parse_scalar_with_ambiguity() {
    // `WITH <ident> AS (<query>)` must stay a standard CTE: the standard form is
    // always attempted first, and it succeeds here.
    let sql = "WITH x AS (SELECT 1) SELECT * FROM x";
    let query = clickhouse_and_generic().verified_query(sql);
    let cte = expect_cte(&with_items(&query, 1)[0]);
    assert_eq!(cte.alias.name, Ident::new("x"));
    assert_eq!(cte.query.to_string(), "SELECT 1");

    // `WITH <ident> AS <ident>` has no parenthesized body, so the standard form
    // fails and this is a scalar declaration binding the *expression* `x` to `y`.
    let sql = "WITH x AS y SELECT y";
    let query = clickhouse_and_generic().verified_query(sql);
    let scalar = expect_scalar(&with_items(&query, 1)[0]);
    assert_eq!(scalar.expr, Expr::Identifier(Ident::new("x")));
    assert_eq!(scalar.alias, Ident::new("y"));

    // A parenthesized scalar subquery is unambiguously the scalar form, because
    // a standard CTE cannot start with `(`.
    let sql = "WITH (SELECT 1) AS x SELECT x";
    let query = clickhouse_and_generic().verified_query(sql);
    let scalar = expect_scalar(&with_items(&query, 1)[0]);
    assert_eq!(scalar.alias, Ident::new("x"));
    assert!(matches!(scalar.expr, Expr::Subquery(_)));
}

#[test]
fn parse_with_item_reports_the_deepest_error() {
    // A malformed standard CTE must not be reported as a malformed scalar item:
    // the CTE attempt consumes far more tokens, so its error is the useful one.
    assert_eq!(
        clickhouse_and_generic()
            .parse_sql_statements("WITH t AS (SELECT 1 FROM) SELECT 1")
            .unwrap_err(),
        ParserError("Expected: identifier, found: )".to_string())
    );
    assert_eq!(
        clickhouse_and_generic()
            .parse_sql_statements("WITH a AS (SELECT 1), b AS (SELECT 2 GROUP) SELECT 1")
            .unwrap_err(),
        ParserError("Expected: ), found: GROUP".to_string())
    );

    // Conversely, an item that is clearly meant to be scalar keeps reporting the
    // scalar error, since the CTE attempt gives up immediately.
    assert_eq!(
        clickhouse_and_generic()
            .parse_sql_statements("WITH 42 AS 43 SELECT 1")
            .unwrap_err(),
        ParserError("Expected: identifier, found: 43".to_string())
    );
}

#[test]
fn parse_scalar_with_unsupported_by_other_dialects() {
    let unsupported = TestedDialects::new(vec![
        Box::new(PostgreSqlDialect {}),
        Box::new(MySqlDialect {}),
    ]);
    assert_eq!(
        unsupported
            .parse_sql_statements("WITH 42 AS magic SELECT magic")
            .unwrap_err(),
        ParserError("Expected: identifier, found: 42".to_string())
    );
    assert_eq!(
        unsupported
            .parse_sql_statements("WITH (SELECT 1) AS wm SELECT wm")
            .unwrap_err(),
        ParserError("Expected: identifier, found: (".to_string())
    );
}

#[test]
fn parse_positional_tuple_access() {
    // Positional (1-based) tuple access, e.g. inside a lambda.
    clickhouse().verified_stmt("SELECT arrayMap(x -> x.2, pairs) FROM analytics.kv_store");
    clickhouse().verified_stmt("SELECT arrayMap(x -> x.2, [('a', 1), ('b', 2)])");

    // Plain positional access on a qualified column.
    clickhouse_and_generic().verified_stmt("SELECT t.1 FROM t");
    // Chained after a regular field access, e.g. `<table>.<tuple column>.<position>`.
    clickhouse_and_generic().verified_stmt("SELECT a.b.2 FROM t");
    // Positional access composes with other expressions.
    clickhouse_and_generic().verified_stmt("SELECT t.2 + 1 AS c FROM t");
    clickhouse_and_generic().verified_stmt("SELECT t.2[1] FROM t");
    clickhouse_and_generic().verified_stmt("SELECT t.1, 1.5 FROM t");
    clickhouse_and_generic().verified_stmt("SELECT * FROM t WHERE t.1 > 1.5");
    clickhouse().verified_stmt("SELECT arrayMap(x -> x.1.2, pairs) FROM t");

    // Positional access is most often applied to a function call or to a
    // parenthesized tuple, so a closing `)` or `]` also ends the root expression.
    clickhouse_and_generic().verified_stmt("SELECT tuple(1, 2).1");
    clickhouse_and_generic().verified_stmt("SELECT (1, 2).2");
    clickhouse_and_generic().verified_stmt("SELECT arr[1].2 FROM t");
    clickhouse_and_generic().verified_stmt("SELECT (t.1).2 FROM t");

    // The AST reuses the existing compound field access node, with the
    // position stored as a numeric value.
    let select = clickhouse_and_generic().verified_only_select("SELECT t.2 FROM t");
    assert_eq!(
        select.projection[0],
        UnnamedExpr(Expr::CompoundFieldAccess {
            root: Box::new(Identifier(Ident::new("t"))),
            access_chain: vec![AccessExpr::Dot(Expr::Value(
                Value::Number("2".parse().unwrap(), false).with_empty_span()
            ))],
        })
    );

    // Chained positional access into a nested tuple.
    clickhouse_and_generic().verified_stmt("SELECT t.1.2 FROM t");
    let select = clickhouse_and_generic().verified_only_select("SELECT t.1.2 FROM t");
    assert_eq!(
        select.projection[0],
        UnnamedExpr(Expr::CompoundFieldAccess {
            root: Box::new(Identifier(Ident::new("t"))),
            access_chain: vec![
                AccessExpr::Dot(Expr::Value(
                    Value::Number("1".parse().unwrap(), false).with_empty_span()
                )),
                AccessExpr::Dot(Expr::Value(
                    Value::Number("2".parse().unwrap(), false).with_empty_span()
                )),
            ],
        })
    );

    // Regressions: float literals must not be affected. The canonical form of a
    // numeric literal depends on whether `bigdecimal` normalizes it.
    clickhouse().verified_stmt("SELECT 1.2");
    let (leading_dot, exponent, underscore) = if cfg!(feature = "bigdecimal") {
        ("SELECT 0.2", "SELECT 1200", "SELECT 1000.5")
    } else {
        ("SELECT .2", "SELECT 1.2e3", "SELECT 1_000.5")
    };
    clickhouse().one_statement_parses_to("SELECT .2", leading_dot);
    // A space before the dot keeps the float interpretation, so `.2` is a
    // stray literal rather than a field access.
    assert_eq!(
        clickhouse()
            .parse_sql_statements("SELECT x .2")
            .unwrap_err()
            .to_string(),
        "sql parser error: Expected: end of statement, found: .2"
    );
    clickhouse().one_statement_parses_to("SELECT 1.2e3", exponent);
    clickhouse().one_statement_parses_to("SELECT 1_000.5", underscore);

    // Dialects without the feature keep treating `.2` as a float literal,
    // which yields a syntax error in this position. (Checked one dialect at a
    // time: the exact error text differs between dialects.)
    assert!(TestedDialects::new(vec![Box::new(PostgreSqlDialect {})])
        .parse_sql_statements("SELECT t.2 FROM t")
        .is_err());
    assert!(TestedDialects::new(vec![Box::new(MySqlDialect {})])
        .parse_sql_statements("SELECT t.2 FROM t")
        .is_err());
}

#[test]
fn parse_view_table_function() {
    // `view(SELECT ...)` turns a query into a table. The subquery carries no
    // parentheses of its own -- ClickHouse rejects `view((SELECT 1))`.
    for sql in [
        "SELECT * FROM view(SELECT 1 AS x)",
        "SELECT * FROM view(SELECT a FROM t WHERE b > 1)",
        "SELECT * FROM view(WITH c AS (SELECT 1) SELECT * FROM c)",
        "SELECT * FROM view(SELECT a FROM t) AS v",
    ] {
        clickhouse_and_generic().verified_stmt(sql);
    }

    let select =
        clickhouse_and_generic().verified_only_select("SELECT * FROM view(SELECT a FROM t)");
    match &select.from[0].relation {
        TableFactor::Table {
            name,
            args: Some(args),
            ..
        } => {
            assert_eq!(name.to_string(), "view");
            assert!(args.args.is_empty());
            assert_eq!(
                args.subquery.as_ref().map(|q| q.to_string()),
                Some("SELECT a FROM t".to_string())
            );
        }
        other => panic!("expected a table function, got {other:?}"),
    }

    // The tables the subquery reads are reachable, which is the point: a
    // visitor collecting relations sees `t`.
    #[cfg(feature = "visitor")]
    {
        use sqlparser::ast::{visit_relations, Statement};
        use std::ops::ControlFlow;

        let statements: Vec<Statement> = clickhouse()
            .parse_sql_statements("SELECT * FROM view(SELECT a FROM t)")
            .unwrap();
        let mut relations = vec![];
        let _ = visit_relations(&statements, |name| {
            relations.push(name.to_string());
            ControlFlow::<()>::Continue(())
        });
        assert!(relations.iter().any(|r| r == "t"), "got {relations:?}");
    }

    // A dialect without the feature keeps rejecting a query in that position.
    assert!(TestedDialects::new(vec![Box::new(PostgreSqlDialect {})])
        .parse_sql_statements("SELECT * FROM view(SELECT 1)")
        .is_err());
}

#[test]
fn parse_select_wildcard_apply() {
    // `APPLY` calls a function on every column the wildcard expands to.
    for sql in [
        "SELECT * APPLY(sum) FROM t",
        "SELECT * APPLY(quantile(0.5)) FROM t",
        "SELECT t.* APPLY(sum) FROM t",
        // Transformers chain, applied left to right.
        "SELECT * APPLY(sum) APPLY(toString) FROM t",
        "SELECT * EXCEPT (b) APPLY(sum) FROM t",
    ] {
        clickhouse_and_generic().verified_stmt(sql);
    }

    // ClickHouse accepts the transformer without parentheses, and this keeps
    // whichever spelling was written.
    clickhouse_and_generic().verified_stmt("SELECT * APPLY sum FROM t");

    // A lambda is a transformer too. Checked on ClickHouse only: GenericDialect
    // has no lambdas and reads `x -> x + 1` as a `->` binary operator.
    clickhouse().verified_stmt("SELECT * APPLY(x -> x + 1) FROM t");

    let applies = |sql: &str| match clickhouse_and_generic()
        .verified_only_select(sql)
        .projection
        .first()
        .cloned()
    {
        Some(SelectItem::Wildcard(options)) => options.opt_apply,
        other => panic!("expected a wildcard, got {other:?}"),
    };

    let apply = applies("SELECT * APPLY(sum) FROM t");
    assert_eq!(apply.len(), 1);
    assert_eq!(apply[0].expr.to_string(), "sum");
    assert!(apply[0].parenthesized);

    let apply = applies("SELECT * APPLY sum FROM t");
    assert!(!apply[0].parenthesized);

    let apply = applies("SELECT * APPLY(sum) APPLY(toString) FROM t");
    assert_eq!(
        apply.iter().map(|a| a.expr.to_string()).collect::<Vec<_>>(),
        ["sum", "toString"]
    );

    // A dialect without the feature leaves `APPLY` alone.
    assert!(TestedDialects::new(vec![Box::new(PostgreSqlDialect {})])
        .parse_sql_statements("SELECT * APPLY(sum) FROM t")
        .is_err());
}

#[test]
fn parse_table_final() {
    // `FINAL` merges rows at read time. Without it, `FROM tbl FINAL` parses as
    // a table aliased `FINAL`, which round-trips to `FROM tbl AS FINAL` and
    // silently drops the merge.
    for sql in [
        "SELECT * FROM tbl FINAL",
        "SELECT * FROM db.tbl FINAL",
        "SELECT * FROM tbl AS t FINAL",
        "SELECT * FROM a FINAL JOIN b FINAL ON a.id = b.id",
        "SELECT count() FROM tbl FINAL WHERE x > 0",
    ] {
        clickhouse_and_generic().verified_stmt(sql);
    }

    let from = clickhouse_and_generic()
        .verified_only_select("SELECT * FROM tbl AS t FINAL")
        .from
        .clone();
    match &from[..] {
        [TableWithJoins {
            relation: TableFactor::Table {
                alias, has_final, ..
            },
            ..
        }] => {
            assert!(has_final);
            assert_eq!(alias.as_ref().map(|a| a.name.value.as_str()), Some("t"));
        }
        other => panic!("expected a single table factor, got {other:?}"),
    }

    // ClickHouse rejects the modifier before the alias, and so does this.
    assert!(clickhouse()
        .parse_sql_statements("SELECT * FROM tbl FINAL AS t")
        .is_err());

    // The order is `<name> [AS <alias>] FINAL [SAMPLE ...]`. Found by running
    // ClickHouse's own test corpus: `FINAL SAMPLE 1 / 2` is all over it, and
    // parsing `FINAL` after the sample rejected every one of them.
    clickhouse_and_generic().verified_stmt("SELECT count() FROM tbl FINAL SAMPLE 1 / 2");
    clickhouse_and_generic().verified_stmt("SELECT count() FROM tbl AS t FINAL SAMPLE 1 / 2");
    assert!(clickhouse()
        .parse_sql_statements("SELECT count() FROM tbl SAMPLE 1 / 2 FINAL")
        .is_err());

    // ClickHouse's parser takes `FINAL` after any table expression, including a
    // derived table, and rejects it later during analysis.
    clickhouse_and_generic().verified_stmt("SELECT * FROM (SELECT 1) FINAL");
    clickhouse_and_generic().verified_stmt("SELECT * FROM (SELECT 1) AS t FINAL SAMPLE 1 / 2");
    clickhouse_and_generic().verified_stmt("WITH c AS (SELECT 1) SELECT * FROM c FINAL");

    match &clickhouse_and_generic()
        .verified_only_select("SELECT * FROM (SELECT 1) FINAL")
        .from[0]
        .relation
    {
        TableFactor::Derived { has_final, .. } => assert!(has_final),
        other => panic!("expected a derived table, got {other:?}"),
    }

    // A dialect without the feature keeps reading `FINAL` as an implicit alias.
    let postgres = TestedDialects::new(vec![Box::new(PostgreSqlDialect {})]);
    let select = postgres.verified_only_select("SELECT * FROM tbl AS FINAL");
    match &select.from[..] {
        [TableWithJoins {
            relation: TableFactor::Table {
                alias, has_final, ..
            },
            ..
        }] => {
            assert!(!has_final);
            assert_eq!(alias.as_ref().map(|a| a.name.value.as_str()), Some("FINAL"));
        }
        other => panic!("expected a single table factor, got {other:?}"),
    }
}

#[test]
fn parse_join_strictness() {
    // ClickHouse takes the strictness on either side of the join kind. All of
    // the pairings below were checked against ClickHouse 26.7.1.
    for sql in [
        "SELECT * FROM t ANY JOIN u USING(a)",
        "SELECT * FROM t ALL JOIN u USING(a)",
        "SELECT * FROM t ANY LEFT JOIN u USING(a)",
        "SELECT * FROM t ALL LEFT JOIN u USING(a)",
        "SELECT * FROM t ANY RIGHT JOIN u USING(a)",
        "SELECT * FROM t ANY INNER JOIN u USING(a)",
        "SELECT * FROM t GLOBAL ANY LEFT JOIN u USING(a)",
        "SELECT * FROM t LEFT SEMI JOIN u USING(a)",
        "SELECT * FROM t RIGHT ANTI JOIN u USING(a)",
        "SELECT * FROM t SEMI JOIN u USING(a)",
    ] {
        clickhouse_and_generic().verified_stmt(sql);
    }

    // The kind may come first; ClickHouse's own order is strictness first, so
    // that is what gets rendered back.
    for (written, rendered) in [
        (
            "SELECT * FROM t LEFT ANY JOIN u USING(a)",
            "SELECT * FROM t ANY LEFT JOIN u USING(a)",
        ),
        (
            "SELECT * FROM t LEFT ALL JOIN u USING(a)",
            "SELECT * FROM t ALL LEFT JOIN u USING(a)",
        ),
        (
            "SELECT * FROM t SEMI LEFT JOIN u USING(a)",
            "SELECT * FROM t LEFT SEMI JOIN u USING(a)",
        ),
        (
            "SELECT * FROM t ANTI RIGHT JOIN u USING(a)",
            "SELECT * FROM t RIGHT ANTI JOIN u USING(a)",
        ),
    ] {
        clickhouse_and_generic().one_statement_parses_to(written, rendered);
    }

    let join =
        |sql: &str| clickhouse_and_generic().verified_only_select(sql).from[0].joins[0].clone();

    // Without this, `ANY` was swallowed as an implicit alias of the left table
    // and the join silently became a plain `LEFT JOIN`.
    let any_left = join("SELECT * FROM t ANY LEFT JOIN u USING(a)");
    assert_eq!(any_left.strictness, Some(JoinStrictness::Any));
    assert!(matches!(any_left.join_operator, JoinOperator::Left(_)));

    let all_left = join("SELECT * FROM t ALL LEFT JOIN u USING(a)");
    assert_eq!(all_left.strictness, Some(JoinStrictness::All));

    let plain = join("SELECT * FROM t LEFT JOIN u USING(a)");
    assert_eq!(plain.strictness, None);

    // `SEMI`/`ANTI` stay in the operator, so strictness is free for `ANY`/`ALL`.
    let semi = join("SELECT * FROM t LEFT SEMI JOIN u USING(a)");
    assert_eq!(semi.strictness, None);
    assert!(matches!(semi.join_operator, JoinOperator::LeftSemi(_)));

    // A dialect without the feature keeps reading `ANY` as an alias.
    let postgres = TestedDialects::new(vec![Box::new(PostgreSqlDialect {})]);
    let select = postgres.verified_only_select("SELECT * FROM t AS ANY LEFT JOIN u USING(a)");
    assert_eq!(select.from[0].joins[0].strictness, None);
}

#[test]
fn parse_ternary_operator() {
    // `<condition> ? <then> : <else>`, ClickHouse's spelling of `if(...)`.
    // Groupings below were read off `EXPLAIN SYNTAX` on ClickHouse 26.7.1.
    for sql in [
        "SELECT a ? b : c",
        "SELECT a ? b : c FROM t",
        "SELECT 1 = 1 ? 2 : 3",
        "SELECT 1 ? (2) : (3 ? 4 : 5)",
        "SELECT * FROM t WHERE flag ? x : y",
    ] {
        clickhouse().verified_stmt(sql);
    }

    let ternary = |sql: &str| match clickhouse().verified_expr(sql) {
        Expr::Ternary {
            condition,
            then_branch,
            else_branch,
        } => (*condition, *then_branch, *else_branch),
        other => panic!("expected a ternary, got {other:?}"),
    };

    // The condition binds looser than every binary operator: ClickHouse reads
    // `1 OR 0 ? 2 : 3` as `if(or(1, 0), 2, 3)`, not `1 OR if(0, 2, 3)`.
    let (condition, _, _) = ternary("1 OR 0 ? 2 : 3");
    assert_eq!(condition.to_string(), "1 OR 0");
    let (condition, _, _) = ternary("0 AND 1 ? 2 : 3");
    assert_eq!(condition.to_string(), "0 AND 1");
    let (condition, _, _) = ternary("1 = 1 ? 2 : 3");
    assert_eq!(condition.to_string(), "1 = 1");

    // Both branches take a full expression.
    let (_, then_branch, else_branch) = ternary("1 ? 2 AND 3 : 4 + 10");
    assert_eq!(then_branch.to_string(), "2 AND 3");
    assert_eq!(else_branch.to_string(), "4 + 10");

    // `:` ends the `then` branch rather than starting a member access.
    let (_, then_branch, _) = ternary("1 ? 2 = 2 : 4");
    assert_eq!(then_branch.to_string(), "2 = 2");

    // ClickHouse rejects an unparenthesized ternary inside another one; this
    // accepts it, grouping as C does. Parsing more than the server accepts is
    // harmless, and the alternative -- tracking parenthesis depth to allow
    // `1 ? (2) : (3 ? 4 : 5)` while rejecting this -- buys nothing.
    let (_, then_branch, else_branch) = ternary("1 ? 2 ? 3 : 4 : 5");
    assert_eq!(then_branch.to_string(), "2 ? 3 : 4");
    assert_eq!(else_branch.to_string(), "5");
    let (_, _, else_branch) = ternary("1 ? 2 : 3 ? 4 : 5");
    assert_eq!(else_branch.to_string(), "3 ? 4 : 5");

    // `?` is the operator here, never a bind parameter -- ClickHouse rejects
    // `= ?` too. Dialects without the feature keep their placeholder.
    assert!(clickhouse()
        .parse_sql_statements("SELECT * FROM t WHERE id = ?")
        .is_err());
    let generic = TestedDialects::new(vec![Box::new(GenericDialect {})]);
    generic.verified_stmt("SELECT * FROM t WHERE id = ?");
}

fn clickhouse() -> TestedDialects {
    TestedDialects::new(vec![Box::new(ClickHouseDialect {})])
}

fn clickhouse_and_generic() -> TestedDialects {
    TestedDialects::new(vec![
        Box::new(ClickHouseDialect {}),
        Box::new(GenericDialect {}),
    ])
}
