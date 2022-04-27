async function hi() {
    var request = require('request');
    let resp = await request('https://api.nomics.com/v1/currencies/ticker?key=8c5152abaf2f8adae23448c9b46417cb48020040&ids=BTC,ETH,XRP&interval=1d,30d&convert=EUR&platform-currency=ETH&per-page=100&page=1')
    let json = await resp.json();
    console.log(json);
  }
  
  hi()