# SWFilechain
本專案參考 https://github.com/koshikraj/pynaivechain 改寫
### 運作流程
#### 安裝所需套件
```
pip install -r requirements.txt
```
#### 運行區塊鏈服務
```
PORT=3001 python3 main.py
PORT=3002 PEERS=ws://localhost:3001 python3 main.py
```
#### 測試及客戶端操作流程邏輯

可使用的服務內容及客戶端操作流程邏輯可以參考 [UI_test.ipynb](https://github.com/acute0203/SWFilechain/blob/master/UI_test.ipynb)

#### 檔案上區塊鏈流程
1. 檔案於客戶端使用GZip壓縮。
2. 使用區塊鏈提供的公鑰對壓縮後的檔案進行加密。
3. 使用自己的私鑰對檔案進行數位簽章。
4. 將檔案(base64Data)、數位簽章(signature)、自己的公鑰（public_key）、檔案名稱（certificateOwner）、原始檔案的HASH(fileHash)，上傳至區塊鏈任一節點。
5. 在加入PoolList前會先進行數位簽章的驗證，待驗證完畢後會進行廣播並將檔案加入到自己的PoolList。
6. 待PoolList累積一定數量的檔案後執行出塊（在Pool中的檔案資料數量必須大於0）。

#### 搜尋檔案流程（檔案必須要出塊後才能被搜尋）
1. 當搜尋的Name有被記錄在區塊鏈內時，會以上傳原始檔案的HASH做為紀錄，以上傳單位的Public_key作為該檔案已被該單位做驗證。
2. 將base64Data以區塊鏈上的私鑰進行解密，並將搜尋結果回傳給客戶端。
3. 於客戶端對檔案進行解壓縮，並呈現該檔案內容以及該檔案已經被哪些單位驗證過。

### HTTP API
以下為本專案提供的API
##### Add data to pool
```
curl -H "Content-type:application/json" --data '{"base64Data" : "","signature":"","public_key":"","certificateOwner":"","fileHash":""}' http://localhost:3001/addData
```
##### Get data in pool
```
curl http://localhost:3001/poolList
```
##### Search file name in blockchain (EX:Miles)
```
curl http://localhost:3001/queryBlockByName?queryName=Miles
```
##### Get server's public key
```
curl http://localhost:3001/getPublicKey
```
以下為原專案提供的API
##### Get blockchain
```
curl http://localhost:3001/blocks
```
##### Create block
```
curl -H "Content-type:application/json" --data '{"data" : ""}' http://localhost:3001/mineBlock
``` 
##### Add peer
```
curl -H "Content-type:application/json" --data '{"peer" : "ws://localhost:6001"}' http://localhost:3001/addPeer
```
#### Query connected peers
```
curl http://localhost:3001/peers
```