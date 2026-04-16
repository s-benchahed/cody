; Function declarations
(function_declaration
  name: (identifier) @name) @fn

; Method declarations
(method_declaration
  name: (field_identifier) @name) @method

; Type declarations (structs / interfaces)
(type_spec
  name: (type_identifier) @name) @struct
