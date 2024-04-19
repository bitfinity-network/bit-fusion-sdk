// SPDX-License-Identifier: GPL-2.0-or-later
pragma solidity =0.7.6;
pragma abicoder v2;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract Cashium is ERC20 {
    uint256 public INITIAL_SUPPLY = 1_000_000_000_000_000 * 10 ** 18;

    constructor() ERC20("Cashium", "CSM") {
        _mint(msg.sender, INITIAL_SUPPLY);
    }
}

contract Intellicoin is ERC20 {
    uint256 public INITIAL_SUPPLY = 1_000_000_000_000_000 * 10 ** 18;

    constructor() ERC20("Intellicoin", "ITC") {
        _mint(msg.sender, INITIAL_SUPPLY);
    }
}

contract Arcoin is ERC20 {
    uint256 public INITIAL_SUPPLY = 1_000_000_000_000_000 * 10 ** 18;

    constructor() ERC20("Arcoin", "ARC") {
        _mint(msg.sender, INITIAL_SUPPLY);
    }
}

contract Incoingnito is ERC20 {
    uint256 public INITIAL_SUPPLY = 1_000_000_000_000_000 * 10 ** 18;

    constructor() ERC20("Incoingnito", "ICG") {
        _mint(msg.sender, INITIAL_SUPPLY);
    }
}

contract Coinicious is ERC20 {
    uint256 public INITIAL_SUPPLY = 1_000_000_000_000_000 * 10 ** 18;

    constructor() ERC20("Coinicious", "CNS") {
        _mint(msg.sender, INITIAL_SUPPLY);
    }
}

contract Coinovation is ERC20 {
    uint256 public INITIAL_SUPPLY = 1_000_000_000_000_000 * 10 ** 18;

    constructor() ERC20("Coinovation", "COV") {
        _mint(msg.sender, INITIAL_SUPPLY);
    }
}

contract Coinaro is ERC20 {
    uint256 public INITIAL_SUPPLY = 1_000_000_000_000_000 * 10 ** 18;

    constructor() ERC20("Coinaro", "CNR") {
        _mint(msg.sender, INITIAL_SUPPLY);
    }
}

contract Coinverse is ERC20 {
    uint256 public INITIAL_SUPPLY = 1_000_000_000_000_000 * 10 ** 18;

    constructor() ERC20("Coinverse", "CVS") {
        _mint(msg.sender, INITIAL_SUPPLY);
    }
}
