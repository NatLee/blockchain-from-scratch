# 跟 2018 年原本專案的對應

這個 repo 原本叫 `SW-Project01`，是 2018 年 NatLee 和 [acute0203](https://github.com/acute0203) 一起做的課堂專案：
一個 Django 圖片上傳網站，加上參考 [pynaivechain](https://github.com/koshikraj/pynaivechain) 改寫的區塊鏈節點「SWFilechain」，
用來證明檔案存在過、被哪些單位驗證過。2026 年重寫成這份教材時，原本的每個功能都找到了新的位置，對照如下。

## 為什麼重寫，而不是修

- 原本的示範金鑰（私鑰和密碼）在公開前已經從 git 歷史拿掉，舊程式本來就跑不起來。
- 原本用的 PyCrypto 系列寫法、Django 2.0、Sanic 0.7 都已經過時。
- 使用者希望它變成可以一步一步學區塊鏈的教材，核心改用 Rust，網頁可以直接在瀏覽器裡跑。

新程式是**從頭重寫**的，沒有複製原本的程式碼。沿用的是設計：HTTP API 的名稱和參數、節點之間的訊息種類、FileChain 的流程、查詢和 `cerLevel` 的規則。

## 功能對照

| 原本 | 現在 |
|---|---|
| `SWFilechain/main.py`：Sanic 節點，`PORT`、`PEERS` 環境變數 | `src/node`：axum 節點，同樣的環境變數 |
| `POST /addData` | 同樣的路徑和欄位（`base64Data`、`signature`、`public_key`、`certificateOwner`、`fileHash`、`cerLevel`），多了 `unitName`、`timestamp` |
| `GET /poolList` | 同樣，回傳 mempool 裡的交易 |
| `GET /queryBlockByName?queryName=` | 同樣的路徑和參數（可以給好幾個 `queryName`），回應改成清楚的清單，見下方 |
| `GET /getPublicKey` | 同樣，回傳 `{"public_key": …}`，值換成 X25519 公鑰 |
| `GET /blocks` | 同樣 |
| `POST /mineBlock`（pool 是空的就回 `No data can be packed into block`） | 同樣，訊息一字不差 |
| `POST /addPeer`、`GET /peers` | 同樣 |
| websocket 在 `/`，4 種訊息 | 同樣在 `/`，同樣 4 種訊息（`src/core/src/p2p.rs`） |
| `blockchain.py` 的區塊、鏈、驗證 | `src/core/src/block.rs`、`chain.rs`，第 2、3 章 |
| `query_block_by_name` 的分組和 `cerLevel` 規則 | `src/core/src/filechain.rs` 的 `find_by_name`，第 9 章 |
| 客戶端：gzip、RSA-OAEP + AES-EAX 加密、RSA 簽章 | `filechain::prepare_upload`：gzip、X25519 + HKDF + AES-256-GCM、Ed25519 |
| 節點用私鑰解密查到的檔案 | `filechain::query_by_name` |
| `loc_lookup`：寫死兩把公鑰對應 `AMERICA`、`CHINA` | 單位在紀錄裡簽章宣告 `unitName`；「信任名單」是第 9 章的練習 |
| `server_private_rsa_key.bin` 等金鑰檔（commit 在 repo 裡） | 節點第一次啟動時產生，存在 `data/` 或 docker volume，`.gitignore` 擋住 |
| Django `imgUpload.html`：拖放或選圖、預覽、名稱、驗證等級、上傳 | 網頁「FileChain」頁的「上傳圖片」 |
| Django「Mine Block」按鈕 + sweetalert2 提示 | 網頁「FileChain」頁的「Mine Block（出塊）」按鈕和提示訊息 |
| Django `searchImg.html`、`showImg.html` | 網頁「FileChain」頁的「搜尋」，顯示圖片、驗證單位、等級和次數 |
| `UI_test.ipynb`：上傳整個資料夾、看 pool、看鏈、出塊、查詢並還原 | `cargo run -p chain-node --example ch09-client`，以及 CI 裡的端對端測試 |
| `python manage.py runserver` + 手動起節點 | `docker compose up --build`：3 個節點 + 網頁 |

Django 本身（`testsite/`、`imgTest/` 的資料庫模型和 migration）沒有保留：原本的上傳流程已經不把圖片存進資料庫（相關程式都被註解掉了），網站只是 FileChain 的客戶端，這個角色由新的網頁接手。

## 刻意改變的行為

1. **加上工作量證明、交易和餘額**：原本的節點沒有挖礦難度（跟 naivechain 一樣），區塊裡直接放檔案紀錄。教材需要第 4–6 章，所以現在 `POST /mineBlock` 會做工作量證明，區塊第一筆是給礦工的出塊獎勵，另外多了轉帳交易（`POST /addTransaction`、`GET /balances`）。檔案紀錄是一種交易，不動到餘額。
2. **`/queryBlockByName` 的回應格式**：原本回傳 `{ "<整個檔案的 base64>": { "AMERICA": 2, … } }`，拿檔案內容當 key。現在回傳一個清單，每個元素有 `fileHash`、`gzipData`（解密後還沒解壓縮的檔案）和 `certifications`（單位公鑰、單位名稱、等級、次數）。舊的客戶端本來就因為金鑰和加密方式改變而不能用，所以趁這次改成清楚的格式。
3. **修好 `cerLevel` 的單位清單被重設**：原本第一次遇到某個等級時，會把那個檔案的單位清單整個重設，先前算到的單位就不見了。現在改成合併。
4. **收到鏈不再清空 pool**：原本收到任何鏈的回應都會清空 pool，排隊中的資料就不見了。現在只拿掉已經上鏈的交易，換鏈時被換掉的交易會放回 mempool 重新檢查（第 8 章）。
5. **`/addData` 簽章錯誤時回 400**：原本簽章不對時默默忽略，照樣回傳 pool，客戶端不知道失敗。現在回 400 和原因（格式沿用原本的 `{"status": false, "message": …}`）。
6. **簽整筆紀錄**：原本只簽密文，名稱、雜湊、等級可以在途中被改掉。現在簽整筆紀錄。
7. **比的是累積工作量，不是長度**：原本換成「比較長」的鏈；現在換成「累積工作量比較多」的鏈（第 8 章）。

## 授權和致謝

- 原本的 SWFilechain 參考 [koshikraj/pynaivechain](https://github.com/koshikraj/pynaivechain) 改寫；pynaivechain 是 [lhartikk/naivechain](https://github.com/lhartikk/naivechain) 的 Python 版。兩者都是 Apache License 2.0。
- 現在的程式是重寫的，沒有沿用它們的程式碼，所以整個 repo 維持 MIT 授權（見 [LICENSE](../LICENSE)）。原本參考 pynaivechain 的舊程式碼仍保留在 git 歷史裡（2018 年的 commit）。
- 感謝 acute0203：FileChain 的客戶端流程、`cerLevel` 驗證等級和「Mine Block」按鈕都是他做的，這份教材的第 9 章就是從他的設計出發。
