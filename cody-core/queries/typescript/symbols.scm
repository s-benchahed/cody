; Reuse JS function patterns (tree-sitter-typescript extends JS grammar)
(function_declaration
  name: (identifier) @name) @fn

(variable_declarator
  name: (identifier) @name
  value: (arrow_function)) @fn

(method_definition
  name: (property_identifier) @name) @fn

(class_declaration
  name: (type_identifier) @name) @class

; TypeScript interface
(interface_declaration
  name: (type_identifier) @name) @interface

; TypeScript type alias
(type_alias_declaration
  name: (type_identifier) @name) @type_alias

; Exported declarations
(export_statement
  declaration: (function_declaration
    name: (identifier) @name)) @export_fn

; Abstract class methods
(abstract_method_signature
  name: (property_identifier) @name) @fn

; NestJS/Angular decorators on classes (entry point hints)
(decorator
  (call_expression
    function: (identifier) @decorator_name)) @decorator
