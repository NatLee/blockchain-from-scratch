from django.shortcuts import render
from imgTest.models import IMG
from PIL import Image
import base64
from io import BytesIO
from Crypto.PublicKey import RSA
from Crypto.Random import get_random_bytes
from Crypto.Cipher import AES, PKCS1_OAEP
from base64 import b64encode, b64decode
from Crypto.Hash import SHA,SHA256
from Crypto.Signature import PKCS1_v1_5
import json, os
import requests
import gzip
import zlib,binascii
# Create your views here.
#from IPython.display import Image, display
import os,random,string
import gzip  
import io
from django.http import HttpResponse

ip  =  "localhost"
port = 3001
end_point = "http://" + ip + ":" +str(port)
location = 'AMERICA'
private_key_path = str(port)+"/client_private_rsa_key.bin"

def gz(filename):
    f_in = open(filename, 'rb')  
    f_out = gzip.open(filename + '.gz', 'wb')  
    f_out.writelines(f_in)  
    f_out.close()  
    f_in.close()  
    return filename + '.gz'

def get_publickey_from_server():
    r = requests.get(end_point + "/getPublicKey")
    public_key = bytes(r.json()['public_key'], encoding = 'utf8')
    return public_key

server_public_key = get_publickey_from_server()

def send_to_server(data):
    headers = {'content-type': 'application/json'}
    url = end_point + "/addData"
    params = {}
    return requests.post(url, params=params, data=json.dumps(data), headers=headers)

def CreateRSAKeys():
    #code = location + 'REMOVED-PASSPHRASE' + str(port)
    key = RSA.generate(2048)
    encrypted_key = key.exportKey(pkcs=8, protection="scryptAndAES128-CBC")
    with open(private_key_path, 'wb') as f:
        f.write(encrypted_key)
    with open(public_key_path, 'wb') as f:
        f.write(key.publickey().exportKey())

def calculate_hash(data_to_hash):
    hash_object = SHA256.new(data_to_hash)
    return hash_object.hexdigest()
        
#client side    
def encrypt_client(filename):         
    data = ''
    original_data = b''
    #fnamegz = filename
    fnamegz = gz(filename)
    with open(filename, 'rb') as f:
        original_data = f.read()
    with open(fnamegz, 'rb') as f:
        data = f.read()
    temp_file_name = fnamegz + '.b'
    with open(temp_file_name, 'wb') as out_file:
        recipient_key = RSA.import_key(server_public_key)
        session_key = get_random_bytes(16)
        # Encrypt the session key with the public RSA key
        cipher_rsa = PKCS1_OAEP.new(recipient_key)
        out_file.write(cipher_rsa.encrypt(session_key))
        # Encrypt the data with the AES session key
        cipher_aes = AES.new(session_key, AES.MODE_EAX)
        ciphertext, tag = cipher_aes.encrypt_and_digest(data)
        out_file.write(cipher_aes.nonce)
        out_file.write(tag)
        out_file.write(ciphertext)
    with open(temp_file_name, 'rb') as f:
        data = f.read()
    os.remove(temp_file_name)
    os.remove(fnamegz)
    return original_data,b64encode(data)

#client side
def generate_rsa_sha1_signature(data, private_key):
    hash_obj = SHA.new(data)
    key = RSA.importKey(private_key)
    signer = PKCS1_v1_5.new(key)
    signature = signer.sign(hash_obj)
    return b64encode(signature)


def image_to_base64(image_path):
    img = Image.open(image_path).convert('RGB')
    output_buffer = BytesIO()
    img.save(output_buffer, format='JPEG')
    byte_data = output_buffer.getvalue()
    base64_str = base64.b64encode(byte_data)
    return base64_str

def clientSide(filename): 
    print('Working at client')
    original_data, encrypt_by_server_pub_base64_data = encrypt_client(filename)
    with open(private_key_path) as f:
        private_key = f.read()
    data_signature = generate_rsa_sha1_signature(encrypt_by_server_pub_base64_data, private_key)
    public_key = RSA.importKey(private_key).publickey().exportKey()
    return original_data,{'base64Data' : encrypt_by_server_pub_base64_data.decode('utf-8'), 'signature' : data_signature.decode('utf-8'), 'public_key' : public_key.decode('utf-8')}

def gzdecode(data) :  
    compressedstream = io.BytesIO(data)  
    gziper = gzip.GzipFile(fileobj=compressedstream)    
    data2 = gziper.read()
    return data2   
    
def queryBlockByName(name):
    headers = {'content-type': 'application/json'}
    url = end_point + "/queryBlockByName?queryName=" + name
    print(requests.get(url, headers=headers))
    return requests.get(url, headers=headers).json()

def uploadImg(request): 
    if request.method == 'POST':
        img = request.FILES.get('img')
        #print(img.read())
        cerLevel = request.POST.get('cerLevel','')
        filename =  ''.join(random.choices(string.ascii_uppercase + string.digits, k = 10))
        #filename += '.jpeg'
        with open(filename, 'wb+') as destination:
            for chunk in img.chunks():
                destination.write(chunk)
        original_data , send_to_server_data = clientSide(filename)
        send_to_server_data['certificateOwner'] = request.POST.get('cerOwner',filename)
        send_to_server_data['fileHash'] = calculate_hash(original_data)
        send_to_server_data['cerLevel'] = cerLevel
        #print(send_to_server_data)
        print(send_to_server(send_to_server_data))
        #print(request.FILES.get('img'))
        os.remove(filename)
        """
        img = IMG(img_url=request.FILES.get('img'))
        img.save()
        print("##############")
        print(img.img_url.path)
        img.base64 = image_to_base64(img.img_url.path)
        img.save()
        """
        #im = Image.open(BytesIO(base64.b64decode(img.base64)))
        #im.show()
    return render(request, 'imgupload.html')

def mineBlock(request): 
    if request.method == 'POST':
        headers = {'content-type': 'application/json'}
        url = end_point + "/mineBlock"
        params = {}
        mineResult = requests.post(url, params=params, data=json.dumps({}), headers=headers)
        return HttpResponse(json.dumps({'status':mineResult.status_code}), content_type="application/json")

def searchImg(request):
    return render(request, 'searchImg.html', {})

def showImg(request):
    #imgs = IMG.objects.all()
    queryName = request.GET.get('queryName','')
    if len(queryName) ==0:
        return render(request, 'showImg.html', {}) 
    search_result= queryBlockByName(queryName)
    imgs =[]
    for file_binary in search_result:
        write_file_data = b64decode(bytes(file_binary, encoding = 'utf8'))
        #filename =  ''.join(random.choices(string.ascii_uppercase + string.digits, k = 10))
        #filename += '.jpeg'
        write_file_data = gzdecode(write_file_data)
        imgs.append({'base64Data':b64encode(write_file_data).decode('utf8'),'sig':search_result[file_binary]})

        #display(Image(filename = filename))
        #os.remove(filename)
        #print(search_result[file_binary])

        #imgs.append(write_file_data)
    context = {
        'imgs' : imgs
    }
    return render(request, 'showImg.html', context)
