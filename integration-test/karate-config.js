function fn() {
    // make sore no cookies are passed b/n scenarios. For Single user flow keeping the cookies is a good behavior
    // but in this case testing API with multiple users, the stored cookie may introduce some unwanted glitch.
    karate.configure('cookies', null);
}
