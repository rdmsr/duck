#pragma once

/// This is an example structure
struct MyStruct {
  int a;
};

/// This is another example structure used to demonstrate cross-references
/// between types
struct OtherStruct {
  /// Something
  MyStruct b;
};

/// A function that does nothing
/// - `a`: First parameter
/// - `b`: Second parameter
void function(int a, OtherStruct *b);
