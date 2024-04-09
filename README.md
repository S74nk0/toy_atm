This is a simple TOY ATM implemented in Rust.

It accepts a path to the input csv file with transactions, handles the transactions and prints out the account balances to the standard output in csv format.

The ATM implementation strategy is to use Rusts `newtype` idiom and to enforce compile time guarantees that the rigt type is being used.
Also the `newtype` allows us to define additional behaviour specific to the `Type` properties.
We use `enums` where appropriate to utilise them exaustively and ensure that we handle all the possible cases (inputs, transitions, outputs, errors).
We have taken advantage of the type system to write tests to validate expected behaviour based on the scenarious that came up.
We have also used the property based testing to generate variable input data. With property based testing we might catch an error case for the input we did not anticipate.
The cornerstone of the testing is to ensure the fact that our account balance coresponds to the double entry book keeping for debits and credits. This is the most important property of the account where we know that if the transaction **IS ignored** this must not change the account balance and if the transaction **is NOT ignored** that a balance update must ocur where we need to check if that update is correct.
About 60% of the code are just tests.

