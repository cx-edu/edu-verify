// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "./PoseidonT3.sol";
import "./Halo2Verifier.sol";
import "./ShplonkVerifier.sol";
import "./Pairing.sol";
import { Constants } from "./Constants.sol";

contract CertificateVerifier is Constants {
    using PoseidonT3 for uint[2];
    
    Halo2Verifier public immutable halo2Verifier;
    Verifier public immutable shplonkVerifier;
    uint256 constant FR_SIZE = 32;
    uint256 constant G1_SIZE = 96;
    
    constructor(address _halo2Verifier, address _shplonkVerifier) {
        halo2Verifier = Halo2Verifier(_halo2Verifier);
        shplonkVerifier = Verifier(_shplonkVerifier);
    }

    // Helper function to chunk data into Fr elements
    function chunkDataToFr(bytes memory data) internal pure returns (uint256[] memory) {
        require(data.length % FR_SIZE == 0, "Data length must be multiple of FR_SIZE");
        uint256 numChunks = data.length / FR_SIZE;
        uint256[] memory result = new uint256[](numChunks);
        
        for (uint256 i = 0; i < numChunks; i++) {
            bytes32 chunk;
            assembly {
                chunk := mload(add(add(data, 32), mul(i, FR_SIZE)))
            }
            result[i] = uint256(chunk);
        }
        return result;
    }

    // Helper function to chunk commitment data into G1 points
    function chunkCommitmentToG1(bytes memory data) internal pure returns (Pairing.G1Point[] memory) {
        require(data.length % G1_SIZE == 0, "Commitment length must be multiple of G1_SIZE");
        uint256 numPoints = data.length / G1_SIZE;
        Pairing.G1Point[] memory points = new Pairing.G1Point[](numPoints);
        
        for (uint256 i = 0; i < numPoints; i++) {
            uint256 offset = 32 + i * G1_SIZE;
            uint256 x;
            uint256 y;
            assembly {
                x := mload(add(data, offset))
                y := mload(add(data, add(offset, 32)))
            }
            points[i] = Pairing.G1Point(x, y);
        }
        return points;
    }

    // Helper function to compute compressed value
    function computeCompressedValue(uint256[] memory data, uint256 xi) internal pure returns (uint256) {
        uint256 result = data[0];
        uint256 xiPower = xi;
        
        for (uint256 i = 1; i < data.length; i++) {
            result = addmod(result, mulmod(data[i], xiPower, BABYJUB_P), BABYJUB_P);
            xiPower = mulmod(xiPower, xi, BABYJUB_P);
        }
        return result;
    }

    // Helper function to compute compressed commitment
    function computeCompressedCommitment(
        Pairing.G1Point[] memory commits, 
        uint256 xi
    ) internal view returns (Pairing.G1Point memory) {
        Pairing.G1Point memory result = commits[0];
        uint256 xiPower = xi;
        
        for (uint256 i = 1; i < commits.length; i++) {
            result = Pairing.plus(
                result,
                Pairing.mulScalar(commits[i], xiPower)
            );
            xiPower = mulmod(xiPower, xi, BABYJUB_P);
        }
        return result;
    }

    function verifyData(
        bytes memory jsonData,
        bytes memory commitment,
        bytes memory proof,
        bytes memory halo2Proof,
        bytes memory index,
        bytes memory random,
        bool is_zk
    ) public view returns (bool) {
        // 1. Chunk jsonData into Fr elements
        uint256[] memory data = chunkDataToFr(jsonData);
        
        // 2. Calculate xi = poseidon_hash(data)
        uint[2] memory hashInputs;
        hashInputs[0] = data[0];
        hashInputs[1] = data.length > 1 ? data[1] : 0;
        uint256 xi = hashInputs.hash();
        
        // 3. Compute compressed value
        uint256 compress = computeCompressedValue(data, xi);
        
        // 4. Calculate delta = poseidon_hash(compress)
        hashInputs[0] = compress;
        hashInputs[1] = 0;
        uint256 delta = hashInputs.hash();
        
        // 5. Calculate final compressed data
        uint256 finalCompressData = mulmod(compress, delta, BABYJUB_P);
        
        // 6. Chunk commitments into G1 points
        Pairing.G1Point[] memory commits = chunkCommitmentToG1(commitment);
        
        // 7. Compute compressed commitment
        Pairing.G1Point memory compressCommit = computeCompressedCommitment(commits, xi);
        
        // 8. Verify proof using Shplonk
        Pairing.G1Point memory finalCommit = Pairing.plus(
            compressCommit,
            Pairing.negate(
                Pairing.mulScalar(
                    Pairing.G1Point(SRS_G1_X[0], SRS_G1_Y[0]),
                    finalCompressData
                )
            )
        );
        
        bool proofRes = shplonkVerifier.verify(
            finalCommit,
            abi.decode(proof, (Pairing.G1Point)),
            abi.decode(index, (uint256)),
            0  // Expected value is 0 since we subtracted the final_compress_data
        );

        // 9. Verify ZK proof if required
        bool zkRes = true;
        if (is_zk) {
            (bool success,) = address(halo2Verifier).staticcall(halo2Proof);
            zkRes = success;
        }

        // 10. Return combined result
        return proofRes && zkRes;
    }

    function hexStringToBytes32(string memory s) internal pure returns (bytes32) {
        bytes memory ss = bytes(s);
        require(ss.length == 64, "Invalid hash length");
        
        bytes memory bytesArray = new bytes(32);
        for (uint i = 0; i < 32; i++) {
            bytesArray[i] = bytes1(
                uint8(hexCharToByte(ss[2*i])) * 16 + 
                uint8(hexCharToByte(ss[2*i+1]))
            );
        }
        
        bytes32 result;
        assembly {
            result := mload(add(bytesArray, 32))
        }
        return result;
    }

    function hexCharToByte(bytes1 c) internal pure returns (bytes1) {
        if (uint8(c) >= 48 && uint8(c) <= 57) return bytes1(uint8(c) - 48);
        if (uint8(c) >= 97 && uint8(c) <= 102) return bytes1(uint8(c) - 87);
        if (uint8(c) >= 65 && uint8(c) <= 70) return bytes1(uint8(c) - 55);
        revert("Invalid hex character");
    }

    function getHash(bytes memory data) public pure returns (bytes32) {
        return keccak256(data);
    }
} 