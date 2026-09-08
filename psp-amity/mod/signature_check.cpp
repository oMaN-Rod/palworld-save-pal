#include "signature_check.hpp"

#include <iterator>

namespace amity_sig
{
namespace
{
constexpr ParamSpec kSetPhysicalHealth[] = {
    {L"PhysicalHealth", L"EnumProperty", 1},
};

constexpr ParamSpec kSetFullStomach[] = {
    {L"NextValue", L"FloatProperty", 4},
};

constexpr ParamSpec kFloatReturn[] = {
    {L"ReturnValue", L"FloatProperty", 4},
};

constexpr ParamSpec kBoolReturn[] = {
    {L"ReturnValue", L"BoolProperty", 1},
};

constexpr ParamSpec kWorldContextObjectReturn[] = {
    {L"WorldContextObject", L"ObjectProperty", 8},
    {L"ReturnValue", L"ObjectProperty", 8},
};

constexpr ParamSpec kGetAllPlayerStates[] = {
    {L"WorldContextObject", L"ObjectProperty", 8},
    {L"OutPlayerStates", L"ArrayProperty", 16},
};

constexpr ParamSpec kGetIndividualCharacterParameter[] = {
    {L"IndividualId", L"StructProperty", 48},
    {L"ReturnValue", L"ObjectProperty", 8},
};

constexpr ParamSpec kObjectReturn[] = {
    {L"ReturnValue", L"ObjectProperty", 8},
};

constexpr ParamSpec kIntReturn[] = {
    {L"ReturnValue", L"IntProperty", 4},
};

constexpr ParamSpec kAddItem[] = {
    {L"StaticItemId", L"NameProperty", 8},
    {L"Count", L"IntProperty", 4},
    {L"IsAssignPassive", L"BoolProperty", 1},
    {L"LogDelay", L"FloatProperty", 4},
    {L"bNotifyLog", L"BoolProperty", 1},
    {L"ReturnValue", L"EnumProperty", 1},
};

constexpr ParamSpec kGetPalStorageDataByPlayerUID[] = {
    {L"WorldContextObject", L"ObjectProperty", 8},
    {L"PlayerUId", L"StructProperty", 16},
    {L"ReturnValue", L"ObjectProperty", 8},
};

constexpr ParamSpec kGetSlotBySlotIndex[] = {
    {L"SlotIndex", L"IntProperty", 4},
    {L"ReturnValue", L"ObjectProperty", 8},
};

constexpr ParamSpec kTryGetContainerFromInventoryType[] = {
    {L"inventoryType", L"EnumProperty", 1},
    {L"OutContainer", L"ObjectProperty", 8},
    {L"ReturnValue", L"BoolProperty", 1},
};

constexpr FunctionSpec kPalHealFunctions[] = {
    {"FullRecoveryHP", L"/Script/Pal.PalIndividualCharacterParameter:FullRecoveryHP", nullptr, 0},
    {"SetPhysicalHealth",
     L"/Script/Pal.PalIndividualCharacterParameter:SetPhysicalHealth",
     kSetPhysicalHealth,
     std::size(kSetPhysicalHealth)},
    {"SetFullStomach", L"/Script/Pal.PalIndividualCharacterParameter:SetFullStomach", kSetFullStomach, std::size(kSetFullStomach)},
    {"GetMaxFullStomach", L"/Script/Pal.PalIndividualCharacterParameter:GetMaxFullStomach", kFloatReturn, std::size(kFloatReturn)},
    {"GetMaxSanityValue", L"/Script/Pal.PalIndividualCharacterParameter:GetMaxSanityValue", kFloatReturn, std::size(kFloatReturn)},
    {"IsHPFullRecovered", L"/Script/Pal.PalIndividualCharacterParameter:IsHPFullRecovered", kBoolReturn, std::size(kBoolReturn)},
    {"GetPalStorageDataByPlayerUID",
     L"/Script/Pal.PalUtility:GetPalStorageDataByPlayerUID",
     kGetPalStorageDataByPlayerUID,
     std::size(kGetPalStorageDataByPlayerUID)},
    {"GetSlotBySlotIndex",
     L"/Script/Pal.PalPlayerDataPalStorage:GetSlotBySlotIndex",
     kGetSlotBySlotIndex,
     std::size(kGetSlotBySlotIndex)},
    {"GetHandle", L"/Script/Pal.PalIndividualCharacterSlot:GetHandle", kObjectReturn, std::size(kObjectReturn)},
    {"TryGetIndividualParameter",
     L"/Script/Pal.PalIndividualCharacterHandle:TryGetIndividualParameter",
     kObjectReturn,
     std::size(kObjectReturn)},
};

constexpr FunctionSpec kInventoryFunctions[] = {
    {"GetAllPlayerStates",
     L"/Script/Pal.PalUtility:GetAllPlayerStates",
     kGetAllPlayerStates,
     std::size(kGetAllPlayerStates)},
    {"GetInventoryData", L"/Script/Pal.PalPlayerState:GetInventoryData", kObjectReturn, std::size(kObjectReturn)},
    {"TryGetContainerFromInventoryType",
     L"/Script/Pal.PalPlayerInventoryData:TryGetContainerFromInventoryType",
     kTryGetContainerFromInventoryType,
     std::size(kTryGetContainerFromInventoryType)},
};

constexpr ParamSpec kGetComponentByClass[] = {
    {L"ComponentClass", L"ClassProperty", 8},
    {L"ReturnValue", L"ObjectProperty", 8},
};

constexpr ParamSpec kGetOtomoIndividualHandle[] = {
    {L"SlotIndex", L"IntProperty", 4},
    {L"ReturnValue", L"ObjectProperty", 8},
};

constexpr ParamSpec kGetSlotId[] = {
    {L"ReturnValue", L"StructProperty", 20},
};

constexpr FunctionSpec kPalSlotFunctions[] = {
    {"GetSlotId", L"/Script/Pal.PalIndividualCharacterSlot:GetSlotId", kGetSlotId, std::size(kGetSlotId)},
};

constexpr FunctionSpec kPartyFunctions[] = {
    {"GetPlayerController", L"/Script/Engine.PlayerState:GetPlayerController", kObjectReturn, std::size(kObjectReturn)},
    {"K2_GetPawn", L"/Script/Engine.Controller:K2_GetPawn", kObjectReturn, std::size(kObjectReturn)},
    {"GetComponentByClass",
     L"/Script/Engine.Actor:GetComponentByClass",
     kGetComponentByClass,
     std::size(kGetComponentByClass)},
    {"GetOtomoIndividualHandle",
     L"/Script/Pal.PalOtomoHolderComponentBase:GetOtomoIndividualHandle",
     kGetOtomoIndividualHandle,
     std::size(kGetOtomoIndividualHandle)},
    {"TryGetIndividualParameter",
     L"/Script/Pal.PalIndividualCharacterHandle:TryGetIndividualParameter",
     kObjectReturn,
     std::size(kObjectReturn)},
};

constexpr ParamSpec kGetNetworkTransmitterByPlayerCharacter[] = {
    {L"Player", L"ObjectProperty", 8},
    {L"ReturnValue", L"ObjectProperty", 8},
};

constexpr ParamSpec kRequestMoveToServer[] = {
    {L"RequestID", L"StructProperty", 16},
    {L"To", L"StructProperty", 20},
    {L"Froms", L"ArrayProperty", 16},
};

constexpr ParamSpec kRequestDisposeToServer[] = {
    {L"RequestID", L"StructProperty", 16},
    {L"SlotInfo", L"StructProperty", 24},
};

constexpr FunctionSpec kItemSetSlotFunctions[] = {
    {"GetAllPlayerStates",
     L"/Script/Pal.PalUtility:GetAllPlayerStates",
     kGetAllPlayerStates,
     std::size(kGetAllPlayerStates)},
    {"GetPlayerController", L"/Script/Engine.PlayerState:GetPlayerController", kObjectReturn, std::size(kObjectReturn)},
    {"K2_GetPawn", L"/Script/Engine.Controller:K2_GetPawn", kObjectReturn, std::size(kObjectReturn)},
    {"GetInventoryData", L"/Script/Pal.PalPlayerState:GetInventoryData", kObjectReturn, std::size(kObjectReturn)},
    {"TryGetContainerFromInventoryType",
     L"/Script/Pal.PalPlayerInventoryData:TryGetContainerFromInventoryType",
     kTryGetContainerFromInventoryType,
     std::size(kTryGetContainerFromInventoryType)},
    {"AddItem_ServerInternal", L"/Script/Pal.PalPlayerInventoryData:AddItem_ServerInternal", kAddItem, std::size(kAddItem)},
    {"GetNetworkTransmitterByPlayerCharacter",
     L"/Script/Pal.PalUtility:GetNetworkTransmitterByPlayerCharacter",
     kGetNetworkTransmitterByPlayerCharacter,
     std::size(kGetNetworkTransmitterByPlayerCharacter)},
    {"GetItem", L"/Script/Pal.PalNetworkTransmitter:GetItem", kObjectReturn, std::size(kObjectReturn)},
    {"RequestMove_ToServer",
     L"/Script/Pal.PalNetworkItemComponent:RequestMove_ToServer",
     kRequestMoveToServer,
     std::size(kRequestMoveToServer)},
    {"RequestDispose_ToServer",
     L"/Script/Pal.PalNetworkItemComponent:RequestDispose_ToServer",
     kRequestDisposeToServer,
     std::size(kRequestDisposeToServer)},
};

constexpr ParamSpec kRequestEmptySlot[] = {
    {L"SlotId", L"StructProperty", 20},
};

constexpr ParamSpec kRequestSwap[] = {
    {L"SlotIdA", L"StructProperty", 20},
    {L"SlotIdB", L"StructProperty", 20},
};

constexpr FunctionSpec kPalRemoveFunctions[] = {
    {"GetAllPlayerStates",
     L"/Script/Pal.PalUtility:GetAllPlayerStates",
     kGetAllPlayerStates,
     std::size(kGetAllPlayerStates)},
    {"GetPlayerController", L"/Script/Engine.PlayerState:GetPlayerController", kObjectReturn, std::size(kObjectReturn)},
    {"K2_GetPawn", L"/Script/Engine.Controller:K2_GetPawn", kObjectReturn, std::size(kObjectReturn)},
    {"GetNetworkTransmitterByPlayerCharacter",
     L"/Script/Pal.PalUtility:GetNetworkTransmitterByPlayerCharacter",
     kGetNetworkTransmitterByPlayerCharacter,
     std::size(kGetNetworkTransmitterByPlayerCharacter)},
    {"GetCharacterContainer",
     L"/Script/Pal.PalNetworkTransmitter:GetCharacterContainer",
     kObjectReturn,
     std::size(kObjectReturn)},
    {"GetPalStorageDataByPlayerUID",
     L"/Script/Pal.PalUtility:GetPalStorageDataByPlayerUID",
     kGetPalStorageDataByPlayerUID,
     std::size(kGetPalStorageDataByPlayerUID)},
    {"GetSlotBySlotIndex",
     L"/Script/Pal.PalPlayerDataPalStorage:GetSlotBySlotIndex",
     kGetSlotBySlotIndex,
     std::size(kGetSlotBySlotIndex)},
    {"GetSlotId", L"/Script/Pal.PalIndividualCharacterSlot:GetSlotId", kGetSlotId, std::size(kGetSlotId)},
    {"GetHandle", L"/Script/Pal.PalIndividualCharacterSlot:GetHandle", kObjectReturn, std::size(kObjectReturn)},
    {"TryGetIndividualParameter",
     L"/Script/Pal.PalIndividualCharacterHandle:TryGetIndividualParameter",
     kObjectReturn,
     std::size(kObjectReturn)},
    {"RequestEmptySlot_ToServer_Rep",
     L"/Script/Pal.PalNetworkCharacterContainerComponent:RequestEmptySlot_ToServer_Rep",
     kRequestEmptySlot,
     std::size(kRequestEmptySlot)},
};

constexpr FunctionSpec kPalMoveFunctions[] = {
    {"GetAllPlayerStates",
     L"/Script/Pal.PalUtility:GetAllPlayerStates",
     kGetAllPlayerStates,
     std::size(kGetAllPlayerStates)},
    {"GetPlayerController", L"/Script/Engine.PlayerState:GetPlayerController", kObjectReturn, std::size(kObjectReturn)},
    {"K2_GetPawn", L"/Script/Engine.Controller:K2_GetPawn", kObjectReturn, std::size(kObjectReturn)},
    {"GetNetworkTransmitterByPlayerCharacter",
     L"/Script/Pal.PalUtility:GetNetworkTransmitterByPlayerCharacter",
     kGetNetworkTransmitterByPlayerCharacter,
     std::size(kGetNetworkTransmitterByPlayerCharacter)},
    {"GetCharacterContainer",
     L"/Script/Pal.PalNetworkTransmitter:GetCharacterContainer",
     kObjectReturn,
     std::size(kObjectReturn)},
    {"GetPalStorageDataByPlayerUID",
     L"/Script/Pal.PalUtility:GetPalStorageDataByPlayerUID",
     kGetPalStorageDataByPlayerUID,
     std::size(kGetPalStorageDataByPlayerUID)},
    {"GetSlotBySlotIndex",
     L"/Script/Pal.PalPlayerDataPalStorage:GetSlotBySlotIndex",
     kGetSlotBySlotIndex,
     std::size(kGetSlotBySlotIndex)},
    {"GetSlotId", L"/Script/Pal.PalIndividualCharacterSlot:GetSlotId", kGetSlotId, std::size(kGetSlotId)},
    {"GetHandle", L"/Script/Pal.PalIndividualCharacterSlot:GetHandle", kObjectReturn, std::size(kObjectReturn)},
    {"TryGetIndividualParameter",
     L"/Script/Pal.PalIndividualCharacterHandle:TryGetIndividualParameter",
     kObjectReturn,
     std::size(kObjectReturn)},
    {"RequestSwap_ToServer_Rep",
     L"/Script/Pal.PalNetworkCharacterContainerComponent:RequestSwap_ToServer_Rep",
     kRequestSwap,
     std::size(kRequestSwap)},
};

constexpr ParamSpec kAddOtomoHandleToFreeSlot[] = {
    {L"Handle", L"ObjectProperty", 8},
    {L"ReturnValue", L"BoolProperty", 1},
};

constexpr ParamSpec kCreateIndividualByFixedID[] = {
    {L"ID", L"StructProperty", 48},
    {L"InitParameter", L"StructProperty", 880},
    {L"spawnCallback", L"DelegateProperty", 16},
    {L"ReturnValue", L"ObjectProperty", 8},
};

constexpr ParamSpec kSetupSaveParameter[] = {
    {L"CharacterID", L"NameProperty", 8},
    {L"Level", L"IntProperty", 4},
    {L"OwnerPlayerUId", L"StructProperty", 16},
    {L"outParameter", L"StructProperty", 880},
    {L"ReturnValue", L"BoolProperty", 1},
};

constexpr ParamSpec kGetIndividualHandleFromCharacterParameter[] = {
    {L"Parameter", L"ObjectProperty", 8},
    {L"ReturnValue", L"ObjectProperty", 8},
};

constexpr FunctionSpec kPalAddFunctions[] = {
    {"GetAllPlayerStates",
     L"/Script/Pal.PalUtility:GetAllPlayerStates",
     kGetAllPlayerStates,
     std::size(kGetAllPlayerStates)},
    {"GetPalStorageDataByPlayerUID",
     L"/Script/Pal.PalUtility:GetPalStorageDataByPlayerUID",
     kGetPalStorageDataByPlayerUID,
     std::size(kGetPalStorageDataByPlayerUID)},
    {"GetSlotBySlotIndex",
     L"/Script/Pal.PalPlayerDataPalStorage:GetSlotBySlotIndex",
     kGetSlotBySlotIndex,
     std::size(kGetSlotBySlotIndex)},
    {"GetSlotId", L"/Script/Pal.PalIndividualCharacterSlot:GetSlotId", kGetSlotId, std::size(kGetSlotId)},
    {"GetHandle", L"/Script/Pal.PalIndividualCharacterSlot:GetHandle", kObjectReturn, std::size(kObjectReturn)},
    {"TryGetIndividualParameter",
     L"/Script/Pal.PalIndividualCharacterHandle:TryGetIndividualParameter",
     kObjectReturn,
     std::size(kObjectReturn)},
    {"GetCharacterManager",
     L"/Script/Pal.PalUtility:GetCharacterManager",
     kWorldContextObjectReturn,
     std::size(kWorldContextObjectReturn)},
    {"CreateIndividualByFixedID",
     L"/Script/Pal.PalCharacterManager:CreateIndividualByFixedID",
     kCreateIndividualByFixedID,
     std::size(kCreateIndividualByFixedID)},
    {"GetIndividualCharacterParameter",
     L"/Script/Pal.PalCharacterManager:GetIndividualCharacterParameter",
     kGetIndividualCharacterParameter,
     std::size(kGetIndividualCharacterParameter)},
    {"GetIndividualHandleFromCharacterParameter",
     L"/Script/Pal.PalCharacterManager:GetIndividualHandleFromCharacterParameter",
     kGetIndividualHandleFromCharacterParameter,
     std::size(kGetIndividualHandleFromCharacterParameter)},
    {"GetOtomoIndividualHandle",
     L"/Script/Pal.PalOtomoHolderComponentBase:GetOtomoIndividualHandle",
     kGetOtomoIndividualHandle,
     std::size(kGetOtomoIndividualHandle)},
    {"GetDatabaseCharacterParameter",
     L"/Script/Pal.PalUtility:GetDatabaseCharacterParameter",
     kWorldContextObjectReturn,
     std::size(kWorldContextObjectReturn)},
    {"SetupSaveParameter",
     L"/Script/Pal.PalDatabaseCharacterParameter:SetupSaveParameter",
     kSetupSaveParameter,
     std::size(kSetupSaveParameter)},
    {"GetComponentByClass",
     L"/Script/Engine.Actor:GetComponentByClass",
     kGetComponentByClass,
     std::size(kGetComponentByClass)},
    {"AddOtomoHandleToFreeSlot",
     L"/Script/Pal.PalOtomoHolderComponentBase:AddOtomoHandleToFreeSlot",
     kAddOtomoHandleToFreeSlot,
     std::size(kAddOtomoHandleToFreeSlot)},
    {"GetNetworkTransmitterByPlayerCharacter",
     L"/Script/Pal.PalUtility:GetNetworkTransmitterByPlayerCharacter",
     kGetNetworkTransmitterByPlayerCharacter,
     std::size(kGetNetworkTransmitterByPlayerCharacter)},
    {"GetCharacterContainer",
     L"/Script/Pal.PalNetworkTransmitter:GetCharacterContainer",
     kObjectReturn,
     std::size(kObjectReturn)},
    {"RequestSwap_ToServer_Rep",
     L"/Script/Pal.PalNetworkCharacterContainerComponent:RequestSwap_ToServer_Rep",
     kRequestSwap,
     std::size(kRequestSwap)},
    {"GetPlayerController", L"/Script/Engine.PlayerState:GetPlayerController", kObjectReturn, std::size(kObjectReturn)},
    {"K2_GetPawn", L"/Script/Engine.Controller:K2_GetPawn", kObjectReturn, std::size(kObjectReturn)},
};

constexpr ParamSpec kEquipWaza[] = {
    {L"WazaID", L"EnumProperty", 2},
};

constexpr ParamSpec kAddPassiveSkill[] = {
    {L"AddSkill", L"NameProperty", 8},
    {L"OverrideSkill", L"NameProperty", 8},
};

constexpr ParamSpec kRemovePassiveSkill[] = {
    {L"SkillId", L"NameProperty", 8},
};

constexpr ParamSpec kSetWorkSuitabilityAddRank[] = {
    {L"WorkSuitability", L"EnumProperty", 1},
    {L"addRank", L"IntProperty", 4},
};

constexpr FunctionSpec kPalEditFunctions[] = {
    {"GetAllPlayerStates",
     L"/Script/Pal.PalUtility:GetAllPlayerStates",
     kGetAllPlayerStates,
     std::size(kGetAllPlayerStates)},
    {"GetPalStorageDataByPlayerUID",
     L"/Script/Pal.PalUtility:GetPalStorageDataByPlayerUID",
     kGetPalStorageDataByPlayerUID,
     std::size(kGetPalStorageDataByPlayerUID)},
    {"GetSlotBySlotIndex",
     L"/Script/Pal.PalPlayerDataPalStorage:GetSlotBySlotIndex",
     kGetSlotBySlotIndex,
     std::size(kGetSlotBySlotIndex)},
    {"GetHandle", L"/Script/Pal.PalIndividualCharacterSlot:GetHandle", kObjectReturn, std::size(kObjectReturn)},
    {"TryGetIndividualParameter",
     L"/Script/Pal.PalIndividualCharacterHandle:TryGetIndividualParameter",
     kObjectReturn,
     std::size(kObjectReturn)},
    {"GetCharacterManager",
     L"/Script/Pal.PalUtility:GetCharacterManager",
     kWorldContextObjectReturn,
     std::size(kWorldContextObjectReturn)},
    {"GetIndividualCharacterParameter",
     L"/Script/Pal.PalCharacterManager:GetIndividualCharacterParameter",
     kGetIndividualCharacterParameter,
     std::size(kGetIndividualCharacterParameter)},
    {"GetLevel", L"/Script/Pal.PalIndividualCharacterParameter:GetLevel", kIntReturn, std::size(kIntReturn)},
    {"ClearEquipWaza", L"/Script/Pal.PalIndividualCharacterParameter:ClearEquipWaza", nullptr, 0},
    {"AddEquipWaza", L"/Script/Pal.PalIndividualCharacterParameter:AddEquipWaza", kEquipWaza, std::size(kEquipWaza)},
    {"AddPassiveSkill",
     L"/Script/Pal.PalIndividualCharacterParameter:AddPassiveSkill",
     kAddPassiveSkill,
     std::size(kAddPassiveSkill)},
    {"RemovePassiveSkill",
     L"/Script/Pal.PalIndividualCharacterParameter:RemovePassiveSkill",
     kRemovePassiveSkill,
     std::size(kRemovePassiveSkill)},
    {"SetWorkSuitabilityAddRank",
     L"/Script/Pal.PalIndividualCharacterParameter:SetWorkSuitabilityAddRank",
     kSetWorkSuitabilityAddRank,
     std::size(kSetWorkSuitabilityAddRank)},
    {"FullRecoveryHP", L"/Script/Pal.PalIndividualCharacterParameter:FullRecoveryHP", nullptr, 0},
};

constexpr FunctionSpec kPlayerEditFunctions[] = {
    {"GetAllPlayerStates",
     L"/Script/Pal.PalUtility:GetAllPlayerStates",
     kGetAllPlayerStates,
     std::size(kGetAllPlayerStates)},
    {"GetCharacterManager",
     L"/Script/Pal.PalUtility:GetCharacterManager",
     kWorldContextObjectReturn,
     std::size(kWorldContextObjectReturn)},
    {"GetIndividualCharacterParameter",
     L"/Script/Pal.PalCharacterManager:GetIndividualCharacterParameter",
     kGetIndividualCharacterParameter,
     std::size(kGetIndividualCharacterParameter)},
};

constexpr OpSpec kOps[] = {
    {"pal.heal", kPalHealFunctions, std::size(kPalHealFunctions)},
    {"item.setSlot", kItemSetSlotFunctions, std::size(kItemSetSlotFunctions)},
    {"pal.remove", kPalRemoveFunctions, std::size(kPalRemoveFunctions)},
    {"pal.move", kPalMoveFunctions, std::size(kPalMoveFunctions)},
    {"pal.add", kPalAddFunctions, std::size(kPalAddFunctions)},
    {"pal.edit", kPalEditFunctions, std::size(kPalEditFunctions)},
    {"player.edit", kPlayerEditFunctions, std::size(kPlayerEditFunctions)},
    {"guild.edit", nullptr, 0},
    {"guild.setRole", nullptr, 0},
};

constexpr OpSpec kReadOps[] = {
    {"inventory", kInventoryFunctions, std::size(kInventoryFunctions)},
    {"party", kPartyFunctions, std::size(kPartyFunctions)},
    {"palSlot", kPalSlotFunctions, std::size(kPalSlotFunctions)},
};
}

const OpSpec* op_specs(std::size_t& count)
{
    count = std::size(kOps);
    return kOps;
}

const OpSpec* find_op_spec(const std::string& op)
{
    for (const OpSpec& spec : kOps)
    {
        if (op == spec.op)
        {
            return &spec;
        }
    }
    return nullptr;
}

const OpSpec* read_op_specs(std::size_t& count)
{
    count = std::size(kReadOps);
    return kReadOps;
}

const OpSpec* find_read_op_spec(const std::string& op)
{
    for (const OpSpec& spec : kReadOps)
    {
        if (op == spec.op)
        {
            return &spec;
        }
    }
    return nullptr;
}

bool params_match(const FunctionSpec& spec, const std::vector<ObservedParam>& observed, std::wstring& detail)
{
    if (observed.size() != spec.param_count)
    {
        detail = L"expected " + std::to_wstring(spec.param_count) + L" params, found " + std::to_wstring(observed.size());
        return false;
    }
    for (std::size_t i = 0; i < spec.param_count; ++i)
    {
        const ParamSpec& expected = spec.params[i];
        const ObservedParam& actual = observed[i];
        if (actual.name != expected.name)
        {
            detail = L"param " + std::to_wstring(i) + L": expected " + expected.name + L", found " + actual.name;
            return false;
        }
        if (actual.type != expected.type)
        {
            detail = std::wstring(expected.name) + L": expected " + expected.type + L", found " + actual.type;
            return false;
        }
        if (actual.size != expected.size)
        {
            detail = std::wstring(expected.name) + L": expected size " + std::to_wstring(expected.size) + L", found " +
                      std::to_wstring(actual.size);
            return false;
        }
    }
    return true;
}
}
