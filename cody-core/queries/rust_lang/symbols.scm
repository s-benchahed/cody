; Function items
(function_item
  name: (identifier) @name) @fn

; Impl block methods
(impl_item
  body: (declaration_list
    (function_item
      name: (identifier) @name))) @method

; Struct definitions
(struct_item
  name: (type_identifier) @name) @struct

; Enum definitions
(enum_item
  name: (type_identifier) @name) @enum

; Trait definitions
(trait_item
  name: (type_identifier) @name) @trait

; Public function items
(visibility_modifier) @vis
