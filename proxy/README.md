Phase 1: 
We are writing 
  1. A TCP listener
  2. A per-connection handler
  3. A bidirectional byte pipe.

client -> TCP -> proxy -> TCP -> backend
first we will be doing: 
  - std only 
  - blocking``
  - one thread per connection 
  - later move to tokito, and hyper

We must code out the client for tcp
