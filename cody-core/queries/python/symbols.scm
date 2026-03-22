; Function definitions
(function_definition
  name: (identifier) @name) @fn

; Async function definitions
(decorated_definition
  (function_definition
    name: (identifier) @name)) @decorated_fn

; Class definitions
(class_definition
  name: (identifier) @name) @class

; Methods inside a class body
(class_definition
  body: (block
    (function_definition
      name: (identifier) @name))) @method
