; Function declarations
(function_definition
  name: (name) @name) @fn

; Class declarations
(class_declaration
  name: (name) @name) @class

; Method declarations inside class body
(class_declaration
  body: (declaration_list
    (method_declaration
      name: (name) @name))) @method

; Interface declarations
(interface_declaration
  name: (name) @name) @interface
