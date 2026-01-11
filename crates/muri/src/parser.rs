use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, Expression, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ImportInfo {
    pub source: String,
    pub kind: ImportKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportKind {
    Static,
    Dynamic,
    Require,
    ExportFrom,
    ExportStar,
    SideEffect,
}

#[derive(Debug)]
pub enum ParseError {
    IoError(std::io::Error),
    ParseFailed(String),
}

impl From<std::io::Error> for ParseError {
    fn from(err: std::io::Error) -> Self {
        ParseError::IoError(err)
    }
}

pub fn extract_imports(path: &Path) -> Result<Vec<ImportInfo>, ParseError> {
    let source = std::fs::read_to_string(path)?;
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_default();
    let parsed = Parser::new(&allocator, &source, source_type).parse();

    if parsed.panicked {
        return Err(ParseError::ParseFailed(format!(
            "Parser panicked on {}",
            path.display()
        )));
    }

    let mut imports = Vec::new();

    for stmt in &parsed.program.body {
        extract_from_statement(stmt, &mut imports);
    }

    Ok(imports)
}

fn extract_from_statement(stmt: &Statement, imports: &mut Vec<ImportInfo>) {
    match stmt {
        Statement::ImportDeclaration(decl) => {
            let kind = if decl.specifiers.as_ref().is_some_and(|s| s.is_empty()) {
                ImportKind::SideEffect
            } else {
                ImportKind::Static
            };
            imports.push(ImportInfo {
                source: decl.source.value.to_string(),
                kind,
            });
        }
        Statement::ExportNamedDeclaration(decl) => {
            if let Some(source) = &decl.source {
                imports.push(ImportInfo {
                    source: source.value.to_string(),
                    kind: ImportKind::ExportFrom,
                });
            }
        }
        Statement::ExportAllDeclaration(decl) => {
            imports.push(ImportInfo {
                source: decl.source.value.to_string(),
                kind: ImportKind::ExportStar,
            });
        }
        Statement::ExpressionStatement(expr_stmt) => {
            extract_from_expression(&expr_stmt.expression, imports);
        }
        Statement::VariableDeclaration(var_decl) => {
            for decl in &var_decl.declarations {
                if let Some(init) = &decl.init {
                    extract_from_expression(init, imports);
                }
            }
        }
        Statement::BlockStatement(block) => {
            for stmt in &block.body {
                extract_from_statement(stmt, imports);
            }
        }
        Statement::IfStatement(if_stmt) => {
            extract_from_expression(&if_stmt.test, imports);
            extract_from_statement(&if_stmt.consequent, imports);
            if let Some(alt) = &if_stmt.alternate {
                extract_from_statement(alt, imports);
            }
        }
        Statement::WhileStatement(while_stmt) => {
            extract_from_expression(&while_stmt.test, imports);
            extract_from_statement(&while_stmt.body, imports);
        }
        Statement::ForStatement(for_stmt) => {
            extract_from_statement(&for_stmt.body, imports);
        }
        Statement::ForInStatement(for_in) => {
            extract_from_statement(&for_in.body, imports);
        }
        Statement::ForOfStatement(for_of) => {
            extract_from_statement(&for_of.body, imports);
        }
        Statement::TryStatement(try_stmt) => {
            for stmt in &try_stmt.block.body {
                extract_from_statement(stmt, imports);
            }
            if let Some(handler) = &try_stmt.handler {
                for stmt in &handler.body.body {
                    extract_from_statement(stmt, imports);
                }
            }
            if let Some(finalizer) = &try_stmt.finalizer {
                for stmt in &finalizer.body {
                    extract_from_statement(stmt, imports);
                }
            }
        }
        Statement::SwitchStatement(switch_stmt) => {
            extract_from_expression(&switch_stmt.discriminant, imports);
            for case in &switch_stmt.cases {
                for stmt in &case.consequent {
                    extract_from_statement(stmt, imports);
                }
            }
        }
        Statement::ReturnStatement(ret) => {
            if let Some(arg) = &ret.argument {
                extract_from_expression(arg, imports);
            }
        }
        Statement::FunctionDeclaration(func) => {
            if let Some(body) = &func.body {
                for stmt in &body.statements {
                    extract_from_statement(stmt, imports);
                }
            }
        }
        Statement::ClassDeclaration(class) => {
            for element in &class.body.body {
                if let oxc_ast::ast::ClassElement::MethodDefinition(method) = element {
                    if let Some(body) = &method.value.body {
                        for stmt in &body.statements {
                            extract_from_statement(stmt, imports);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn extract_from_expression(expr: &Expression, imports: &mut Vec<ImportInfo>) {
    match expr {
        Expression::ImportExpression(import_expr) => {
            if let Expression::StringLiteral(lit) = &import_expr.source {
                imports.push(ImportInfo {
                    source: lit.value.to_string(),
                    kind: ImportKind::Dynamic,
                });
            }
        }
        Expression::CallExpression(call) => {
            // Check for require("...")
            if let Expression::Identifier(ident) = &call.callee {
                if ident.name == "require" {
                    if let Some(Argument::StringLiteral(lit)) = call.arguments.first() {
                        imports.push(ImportInfo {
                            source: lit.value.to_string(),
                            kind: ImportKind::Require,
                        });
                    }
                }
            }
            // Recurse into callee and arguments
            extract_from_expression(&call.callee, imports);
            for arg in &call.arguments {
                if let Argument::SpreadElement(spread) = arg {
                    extract_from_expression(&spread.argument, imports);
                } else if let Some(expr) = arg.as_expression() {
                    extract_from_expression(expr, imports);
                }
            }
        }
        Expression::ArrowFunctionExpression(arrow) => {
            for stmt in &arrow.body.statements {
                extract_from_statement(stmt, imports);
            }
        }
        Expression::FunctionExpression(func) => {
            if let Some(body) = &func.body {
                for stmt in &body.statements {
                    extract_from_statement(stmt, imports);
                }
            }
        }
        Expression::ConditionalExpression(cond) => {
            extract_from_expression(&cond.test, imports);
            extract_from_expression(&cond.consequent, imports);
            extract_from_expression(&cond.alternate, imports);
        }
        Expression::SequenceExpression(seq) => {
            for expr in &seq.expressions {
                extract_from_expression(expr, imports);
            }
        }
        Expression::AssignmentExpression(assign) => {
            extract_from_expression(&assign.right, imports);
        }
        Expression::LogicalExpression(logical) => {
            extract_from_expression(&logical.left, imports);
            extract_from_expression(&logical.right, imports);
        }
        Expression::BinaryExpression(binary) => {
            extract_from_expression(&binary.left, imports);
            extract_from_expression(&binary.right, imports);
        }
        Expression::UnaryExpression(unary) => {
            extract_from_expression(&unary.argument, imports);
        }
        Expression::AwaitExpression(await_expr) => {
            extract_from_expression(&await_expr.argument, imports);
        }
        Expression::ParenthesizedExpression(paren) => {
            extract_from_expression(&paren.expression, imports);
        }
        Expression::ArrayExpression(arr) => {
            for elem in &arr.elements {
                if let Some(expr) = elem.as_expression() {
                    extract_from_expression(expr, imports);
                }
            }
        }
        Expression::ObjectExpression(obj) => {
            for prop in &obj.properties {
                if let oxc_ast::ast::ObjectPropertyKind::ObjectProperty(p) = prop {
                    extract_from_expression(&p.value, imports);
                }
            }
        }
        _ => {
            // Handle member expressions using the helper method
            if let Some(member) = expr.as_member_expression() {
                extract_from_expression(member.object(), imports);
            }
        }
    }
}
