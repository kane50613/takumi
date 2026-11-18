# Introduction

The files in this folder track the properties that are required to achieve
different compliance level, as well as a description of how/why krilla achieves those.
As krilla evolves and new features are added, these documents
might have to be updated, and we always need to ensure that all invariants are still enforced,
when adding new features.

# Legend

🟢: This means that krilla actively checks that this property is enforced, either by an
invariant in the code, or by returning an error to the user in case it's not fulfillable.

🔵: This means that krilla fulfills this property because it's not supported.

🟣: This means that this property cannot be enforced by krilla, and thus is only documented.  
It is upon the user of the library to enforce it.

🔴: This means that the property is currently not enforced by krilla.

🟠: This means that the property should perhaps be documented/checked, but is
currently not because it seems unnecessary or hard to implement.

-: This means that the clause is not applicable to krilla (for example because it's not a reader application)
