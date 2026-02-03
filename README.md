# WebiWabo

## How to test

Currenttly the project contains a `.env.example`, the first step is create a `.env` file, in that file you have to copy the variable `TEST_COOKIE`, then you need to specify the cookie for visualize the components according to your feed, for get the cookie, the easiest way is:

- Open a private tab in your browser
- Go to youtube music
- Open de developer tools
- In the developer tools go to `Navigation` tab
- Log in
- In the search bar in the Navigation section search "/browse"
- Scroll down until find the Cokie fild and copy the entire value in the .env variable

Once you maded the process for get the cookie you can test only executing:

```
just
```
