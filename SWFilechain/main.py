#coding=utf-8
import json as JSON
import os
import websockets
import random,string,binascii
from Crypto.Hash import SHA256
from sanic import Sanic
from sanic.response import json
from websockets.exceptions import ConnectionClosed

from blockchain import Block, Blockchain
from utils.logger import logger
from Crypto.PublicKey import RSA
from Crypto.Random import get_random_bytes
from Crypto.Cipher import AES, PKCS1_OAEP
from base64 import b64encode, b64decode
from Crypto.Hash import SHA
from Crypto.Signature import PKCS1_v1_5
from collections import Counter

QUERY_LATEST = 0
QUERY_ALL = 1
RESPONSE_BLOCKCHAIN = 2
BROADCASTED_MESSAGE = 3

try:
    port = int(os.environ['PORT'])
except KeyError as e:
    port = 3001

try:
    initialPeers = os.environ['PEERS'].split(",")
except Exception as e:
    initialPeers = []


class Server(object):

    def __init__(self):
        self.app = Sanic()
        self.app.config.REQUEST_MAX_SIZE = 20971520
        self.app.config.WEBSOCKET_MAX_SIZE = None
        #print(self.app.config)
        self.loc_lookup = {'-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAqGWZQ+9jibFFEl9TBOQn\n2y1jHlc73FuiWu7+i5hQTyBgBqqV20gYUt6pY9oz4bCuKPJqAjC6EVOrL7/GcIzK\ncHY9FM/cQ/hd+ld/qmb3VNJ96FycQ2P7IbVqcsm5SsuQJLRA3uDIjXMS60I4XoB+\n+HH7U+zcMgQp8esbFHXVuQbitEeDFa3CwWPC68B8dRwmmAR2kqvLULP6qwu5vgkj\nMfxWLPlwVrRF6lMdj9xQW3Q3tF1L2/5YQZD8WyGfvp+6llnciZZwjXnlqMvFj/wZ\nbgo7tbiNCSyU5A0b/g1eHpXXOkbFg/17VNzlIknm1x63/P3XesT0pB9ow/CLPGtL\nZQIDAQAB\n-----END PUBLIC KEY-----':'CHINA', '-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA1w75Qg+eS03vC2FWv88f\nUV5vMh7tnyxOats7WCmMpKcObvM6EfH+j0quHQSbARCvUHsbzr7xInbScWrO5uEZ\n73ZJzLQpcYYDRjvMCJt+HDALjH76wbg3tschGvpHrwuum7V/onTT5DZtNb9OMSVs\nsJihS2khzDsbo5Xvqb34uJNPt1PGhH+WmzXCMt27KvRiBo7QhvBybOMgJnPcpOKe\nsHhm/lb1D79eNo4zRkao/5fzZxGpyZybs+K7zfL6hkzgh3zghTtqmF49BWk4tYk+\n5hsR0UCBtxMeYM2EQktYxgIpfGd+MOR6GrScPRj/6LhBmx0Kj4rXbBIDJmwOuofZ\nOwIDAQAB\n-----END PUBLIC KEY-----':'AMERICA'}
        self.blockchain = Blockchain()
        self.sockets = []
        self.pool = []
        self.app.add_route(self.blocks, '/blocks', methods=['GET'])
        self.app.add_route(self.mine_block, '/mineBlock', methods=['POST'])
        self.app.add_route(self.peers, '/peers', methods=['GET'])
        self.app.add_route(self.add_peer, '/addPeer', methods=['POST'])
        self.app.add_route(self.add_data, '/addData', methods=['POST'])
        self.app.add_route(self.query_block_by_name, '/queryBlockByName', methods=['GET'])
        self.app.add_route(self.pool_list, '/poolList', methods=['GET'])
        self.app.add_route(self.get_public_key, '/getPublicKey', methods=['GET'])
        self.app.add_websocket_route(self.p2p_handler, '/')

    async def query_block_by_name(self, request):
        returnResult = {}
        for queryName in request.args['queryName']:
            query_result = self.blockchain.query_block_by_name(queryName)
            for query_hash in query_result:
                query_file_bytes = bytes(query_result[query_hash]['b64file'], encoding = 'utf8')
                data = b64decode(query_file_bytes)
                print(self.descrypt_server(data))
                returnResult[b64encode(self.descrypt_server(data)).decode('utf8')] = dict(Counter(query_result[query_hash]['public_key']))
        for ff in returnResult:
            pk_to_loc_dict = {}
            for pk_dict in returnResult[ff]:
                if pk_dict in self.loc_lookup:
                    pk_to_loc_dict[self.loc_lookup[pk_dict]] = returnResult[ff][pk_dict]
                else:
                    pk_to_loc_dict[pk_dict] = returnResult[ff][pk_dict]
            returnResult[ff] = pk_to_loc_dict
        return json(returnResult)


    async def get_public_key(self, request):
        with open('server_rsa_public.pem') as f:
            public_key = f.read()
        return json({"public_key" : public_key})

    async def pool_list(self, request):
        return json(self.pool)

    async def add_data(self, request):
        logger.info('78:Received message: {}'.format(request.json))
        loadJSON = request.json
        #print(loadJSON['public_key'])
        signature = bytes(loadJSON['signature'], encoding = 'utf8')
        base64Data = bytes(loadJSON['base64Data'], encoding = 'utf8')
        public_key = bytes(loadJSON['public_key'], encoding = 'utf8')
        if self.verify_rsa_sha1_signature(base64Data, public_key, signature):
            data_list = [loadJSON["certificateOwner"],loadJSON['base64Data'],loadJSON['public_key'],loadJSON['fileHash']]
            self.pool.append(data_list)
            await self.broadcast(self.response_broadcast_msg(data_list))
        return json(self.pool)

    async def blocks(self, request):
        return json(self.blockchain.blocks)

    async def mine_block(self, request):
        if len(self.pool) == 0:
            return json({"status": False, "message": "No data can be packed into block"})
        #self.pool.append("This data is created by miner. port:" + str(port))
        try:
            newBlock = self.blockchain.generate_next_block(self.pool)
            self.pool = []
        except KeyError as e:
            return json({"status": False, "message": "pass value in data key"})
        self.blockchain.add_block(newBlock)
        await self.broadcast(self.response_latest_msg())
        return json(newBlock)

    async def peers(self, request):
        peers = map(lambda x: "{}:{}".format(x.remote_address[0], x.remote_address[1])
                    , self.sockets)
        return json(peers)

    async def add_peer(self, request):
        import asyncio
        asyncio.ensure_future(self.connect_to_peers([request.json["peer"]]),
                                             loop=asyncio.get_event_loop())
        return json({"status": True})

    async def connect_to_peers(self, newPeers):
        for peer in newPeers:
            logger.info(peer)
            try:
                ws = await websockets.connect(peer, max_size = None)

                await self.init_connection(ws)
            except Exception as e:
                logger.info(str(e))

    # initP2PServer WebSocket server
    async def p2p_handler(self, request, ws):
        logger.info('listening websocket p2p port on: %d' % port)


        try:
            await self.init_connection(ws)
        except (ConnectionClosed):
            await self.connection_closed(ws)

    async def connection_closed(self, ws):

        logger.critical("connection failed to peer")
        self.sockets.remove(ws)

    async def init_connection(self, ws):

        self.sockets.append(ws)
        await ws.send(JSON.dumps(self.query_chain_length_msg()))

        while True:
            await self.init_message_handler(ws)

    async def init_message_handler(self, ws):
        data = await ws.recv()
        message = JSON.loads(data)
        logger.info('157:Received message: {}'.format(data))
        if int(message['type']) == BROADCASTED_MESSAGE:
            for mes in message['data']:
                if mes not in self.pool:
                    self.pool.append(mes)
                    await self.broadcast(self.response_broadcast_msg(mes))
        else:
            await {
                QUERY_LATEST: self.send_latest_msg,
                QUERY_ALL: self.send_chain_msg,
                RESPONSE_BLOCKCHAIN: self.handle_blockchain_response
            }[message["type"]](ws, message)

    async def send_latest_msg(self, ws, *args):
        await ws.send(JSON.dumps(self.response_latest_msg()))

    async def send_chain_msg(self, ws, *args):

        await ws.send(JSON.dumps(self.response_chain_msg()))


    def response_broadcast_msg(self, message):
        return {
            'type': BROADCASTED_MESSAGE,
            'data': [message]
        }

    def response_chain_msg(self):
        return {
            'type': RESPONSE_BLOCKCHAIN,
            'data': JSON.dumps([block.dict() for block in self.blockchain.blocks])
        }

    def response_latest_msg(self):

        return {
            'type': RESPONSE_BLOCKCHAIN,
            'data': JSON.dumps([self.blockchain.get_latest_block().dict()])
        }

    async def handle_blockchain_response(self, ws, message):
        self.pool = []
        received_blocks = sorted(JSON.loads(message["data"]), key=lambda k: k['index'])
        logger.info(received_blocks)
        latest_block_received = received_blocks[-1]
        latest_block_held = self.blockchain.get_latest_block()
        if latest_block_received["index"] > latest_block_held.index:
            logger.info('blockchain possibly behind. We got: ' + str(latest_block_held.index)
                  + ' Peer got: ' + str(latest_block_received["index"]))
            if latest_block_held.hash == latest_block_received["previous_hash"]:
                logger.info("We can append the received block to our chain")

                self.blockchain.blocks.append(Block(**latest_block_received))
                await self.broadcast(self.response_latest_msg())
            elif len(received_blocks) == 1:
                logger.info("We have to query the chain from our peer")
                await self.broadcast(self.query_all_msg())
            else:
                logger.info("Received blockchain is longer than current blockchain")
                await self.replace_chain(received_blocks)
        else:
            logger.info('received blockchain is not longer than current blockchain. Do nothing')

    async def replace_chain(self, newBlocks):

        try:

            if self.blockchain.is_valid_chain(newBlocks) and len(newBlocks) > len(self.blockchain.blocks):
                logger.info('Received blockchain is valid. Replacing current blockchain with '
                            'received blockchain')
                self.blockchain.blocks = [Block(**block) for block in newBlocks]
                await self.broadcast(self.response_latest_msg())
            else:
                logger.info('Received blockchain invalid')
        except Exception as e:
            logger.info("Error in replace chain" + str(e))



    def query_chain_length_msg(self):

        return {'type': QUERY_LATEST}

    def query_all_msg(self):

        return {'type': QUERY_ALL}

    async def broadcast(self, message):

        for socket in self.sockets:
            logger.info(socket)
            await socket.send(JSON.dumps(message))

    #server side
    def verify_rsa_sha1_signature(self, data, public_key, signature):
        hash_obj = SHA.new(data)
        key = RSA.importKey(public_key)
        signature = b64decode(signature)
        verifier = PKCS1_v1_5.new(key)
        return verifier.verify(hash_obj, signature)

    #server side
    def descrypt_server(self, data):
        filename = ''.join(random.choices(string.ascii_uppercase + string.digits, k = 10))
        print("create file:" + filename)
        code = 'REMOVED-PASSPHRASE'
        with open(filename, 'wb') as temp:
            temp.write(data)
        with open(filename, 'rb') as fobj:
            private_key = RSA.import_key(open('server_private_rsa_key.bin').read(), passphrase=code)
            enc_session_key, nonce, tag, ciphertext = [ fobj.read(x)
                                                        for x in (private_key.size_in_bytes(),
                                                        16, 16, -1) ]
            cipher_rsa = PKCS1_OAEP.new(private_key)
            session_key = cipher_rsa.decrypt(enc_session_key)
            cipher_aes = AES.new(session_key, AES.MODE_EAX, nonce)
            data = cipher_aes.decrypt_and_verify(ciphertext, tag)
        os.remove(filename)
        return(data)

if __name__ == '__main__':

    server = Server()
    server.app.add_task(server.connect_to_peers(initialPeers))
    server.app.run(host='0.0.0.0', port=port, debug=True)
