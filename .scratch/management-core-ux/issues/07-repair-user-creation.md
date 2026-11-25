# 07: Repair User creation

**What to build:** An authorized administrator can create a User through one accessible form with native validation and Enter submission. Invalid data never reaches the API.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Name, Email, Password, and actions belong to one real form with `new-password` autocomplete.
- [ ] Invalid email and short-password submissions produce field guidance and no API request.
- [ ] Enter sends exactly one valid request, and an authorized create succeeds through the existing contract.
- [ ] Failed requests preserve valid draft values and show useful guidance without raw `ResponseError` text.
- [ ] Real-backend tests cover authorized creation and sign-in, plus invalid and unauthorized paths.
