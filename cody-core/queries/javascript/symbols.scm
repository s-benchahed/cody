; Named function declarations
(function_declaration
  name: (identifier) @name) @fn

; Arrow functions assigned to variables
(variable_declarator
  name: (identifier) @name
  value: (arrow_function)) @fn

; Method definitions inside class bodies
(method_definition
  name: (property_identifier) @name) @fn

; Class declarations
(class_declaration
  name: (identifier) @name) @class

; Exported function declarations
(export_statement
  declaration: (function_declaration
    name: (identifier) @name)) @export_fn

; Exported arrow functions
(export_statement
  declaration: (variable_declaration
    (variable_declarator
      name: (identifier) @name
      value: (arrow_function)))) @export_fn
