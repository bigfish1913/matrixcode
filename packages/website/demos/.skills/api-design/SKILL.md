---
name: api-design
description: RESTful API design guidance
trigger: User needs to design API endpoints
priority: implementation
type: flexible
---

# RESTful API Design Skill

## When to use
- User says "design API", "create endpoints"
- User describes functionality to expose

## Design principles

1. Resource naming
   - Use nouns, not verbs
   - Use plural: /users, /posts

2. HTTP methods
   - GET: retrieve
   - POST: create
   - PUT: update
   - DELETE: delete

3. Status codes
   - 200: success
   - 201: created
   - 400: bad request
   - 404: not found

## Workflow

1. Understand requirements
2. Design routes
3. Define responses
4. Implement handlers
5. Add tests

## Example

```rust
// GET /api/v1/users/:id
async fn get_user(id: u64) -> Result<User, Error> {
    User::find(id)
}
```
