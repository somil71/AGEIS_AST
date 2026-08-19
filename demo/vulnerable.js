function getUserInput() {
    let userInput = document.getElementById("search-box").value;
    // VULNERABLE: Direct innerHTML injection
    document.getElementById("results").innerHTML = userInput;
}
